use crate::tool::ffprobe::FileInfo;
use anyhow::{anyhow, bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
use regex::Regex;
use std::env::consts::OS;
use std::env::current_dir;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{ChildStderr, Command, Stdio};
use std::time::Duration;

lazy_static! {
    static ref RE_DURATION: Regex = Regex::new(r#"^\s*out_time_ms\s*=\s*(\d+).*$"#).unwrap();
}

pub struct FFmpeg {
    cmd: Command,
}

impl FFmpeg {
    pub fn new(input_file: &Path) -> Self {
        let mut path = current_dir().unwrap_or_default();
        let ffmpeg = match OS {
            "windows" => "ffmpeg.exe",
            _ => "ffmpeg",
        };

        path.push(ffmpeg);

        if !Path::new(&path).exists() {
            path.clear();
            path.push(ffmpeg);
        }

        let mut ffmpeg = FFmpeg {
            cmd: Command::new(path),
        };

        ffmpeg
            .cmd
            // send program-friendly progress information to stdout
            .arg("-progress")
            .arg("-")
            // disable print encoding progress/statistics
            .arg("-nostats")
            // explicitly disable interaction you need to specify
            .arg("-nostdin")
            // suppress printing banner
            .arg("-hide_banner")
            // input file
            .arg("-i")
            .arg(input_file);

        ffmpeg
    }

    pub fn cmd(&mut self) -> &mut Command {
        &mut self.cmd
    }

    fn dump_command_args(&self) {
        println!("Running ffmpeg with the following arguments:");
        print!("[ ");
        self.cmd
            .get_args()
            .for_each(|arg| print!("{} ", arg.to_str().unwrap_or_default()));
        println!("]");
    }

    pub fn add_common_args(&mut self, file_info: &FileInfo, ffmpeg_args: &[String]) {
        // set bit rate
        if let Some(bitrate) = file_info.bit_rate() {
            self.cmd.arg("-b:a").arg(format!("{}", bitrate));
        }

        // set codec name
        if let Some(codec) = file_info.codec_name() {
            self.cmd.arg("-c:a").arg(codec);
        }

        // custom args
        ffmpeg_args.iter().for_each(|arg| {
            self.cmd.arg(arg);
        });
    }

    pub fn exec(
        &mut self,
        info_msg: &str,
        verbose: bool,
        duration: Option<Duration>,
    ) -> Result<BufReader<ChildStderr>> {
        self.cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        println!("{info_msg}");

        if verbose {
            self.dump_command_args();
        }

        let mut child = self
            .cmd
            .spawn()
            .with_context(|| "Failed to run ffmpeg tool")?;

        let bar = ProgressBar::new(
            duration
                .unwrap_or_else(|| Duration::from_secs(10))
                .as_micros() as u64,
        );

        if duration.is_some() {
            bar.set_style(
                ProgressStyle::default_bar().template(
                    "[{elapsed_precise}] {bar:50.cyan/cyan} {percent}% (estimated: {eta})",
                ),
            );
        } else {
            bar.set_style(
                ProgressStyle::default_bar().template("[{elapsed_precise}] {spinner:.cyan}"),
            );
        }
        bar.set_position(0);

        if let Some(stdout) = child.stdout.take() {
            BufReader::new(stdout)
                .lines()
                .filter_map(|line| line.ok())
                .for_each(|line| {
                    if line == "progress=end" {
                        bar.finish();
                    } else if let Some(ms) = RE_DURATION
                        .captures(line.as_str())
                        .and_then(|m| m.get(1).map(|m| m.as_str()))
                        .and_then(|ms| ms.parse::<u64>().ok())
                    {
                        if duration.is_some() {
                            bar.set_position(ms);
                        } else {
                            bar.set_position(ms % 10);
                        }
                    }
                });
        } else {
            bail!("Failed to open ffmpeg stdout");
        }

        _ = child.wait();

        Ok(BufReader::new(
            child
                .stderr
                .take()
                .ok_or_else(|| anyhow!("Failed to open ffmpeg stderr"))?,
        ))
    }
}
