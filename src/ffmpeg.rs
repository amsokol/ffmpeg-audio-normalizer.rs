use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
use regex::Regex;
use std::env::consts::OS;
use std::env::current_dir;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

#[derive(Debug, Default)]
pub struct EbuLoudnessValues {
    pub input_i: Option<f64>,
    pub input_lra: Option<f64>,
    pub input_tp: Option<f64>,
    pub input_thresh: Option<f64>,
    pub output_i: Option<f64>,
    pub output_lra: Option<f64>,
    pub output_tp: Option<f64>,
    pub output_thresh: Option<f64>,
    pub normalization_type: Option<String>,
    pub target_offset: Option<f64>,
}

lazy_static! {
    static ref RE_DURATION: Regex = Regex::new(r#"^\s*out_time_ms\s*=\s*(\d+).*$"#).unwrap();
    static ref RE_VALUES: Regex = Regex::new(r#"^\s*"(\S+)"\s*:\s*"(\S+)",?\s*$"#).unwrap();
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

    pub fn exec(
        &mut self,
        info_msg: &str,
        verbose: bool,
        duration: Option<Duration>,
    ) -> Result<EbuLoudnessValues> {
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

        BufReader::new(
            child
                .stdout
                .take()
                .with_context(|| "Failed to open ffmpeg stdout")?,
        )
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

        let values = FFmpeg::result(BufReader::new(
            child
                .stderr
                .take()
                .with_context(|| "Failed to open ffmpeg stderr")?,
        ))
        .with_context(|| "Failed to get ffmpeg execution result")?;

        Ok(values)
    }

    pub fn result<R: Read>(reader: BufReader<R>) -> Result<EbuLoudnessValues> {
        let mut is_json = false;
        let mut err_log = String::new();
        let mut err_parse = String::new();
        let mut values = EbuLoudnessValues {
            ..Default::default()
        };
        let mut values_count = 0;

        reader
            .lines()
            .filter_map(|line| line.ok())
            .filter(|line| match line.as_str() {
                "{" => {
                    is_json = true;
                    false
                }
                "}" => {
                    is_json = false;
                    false
                }
                _ => {
                    if is_json {
                        true
                    } else {
                        // log error in case of problems
                        err_log += line;
                        err_log += "\n";

                        false
                    }
                }
            })
            .for_each(|line| {
                let caps = RE_VALUES.captures(&line);

                if caps.is_none() {
                    err_parse += &format!("Failed to parse loudness value: {}\n", line);
                    return;
                }

                let m = caps.unwrap();
                let field = m.get(1).map_or("", |m| m.as_str());
                let value_str = String::from(m.get(2).map_or("", |m| m.as_str()));

                match field {
                    "normalization_type" => {
                        values.normalization_type = Some(value_str);
                        values_count += 1;
                    }
                    _ => {
                        match value_str.parse::<f64>() {
                            Ok(value) => {
                                match field {
                                    "input_i" => values.input_i = Some(value),
                                    "input_tp" => values.input_tp = Some(value),
                                    "input_lra" => values.input_lra = Some(value),
                                    "input_thresh" => values.input_thresh = Some(value),
                                    "output_i" => values.output_i = Some(value),
                                    "output_tp" => values.output_tp = Some(value),
                                    "output_lra" => values.output_lra = Some(value),
                                    "output_thresh" => values.output_thresh = Some(value),
                                    "target_offset" => values.target_offset = Some(value),
                                    _ => {
                                        err_parse += &format!("Unknown loudness value: {}\n", line);
                                        return;
                                    }
                                }
                                values_count += 1;
                            }
                            Err(_) => {
                                err_parse += &format!("Invalid loudness value: {}\n", line);
                            }
                        };
                    }
                }
            });

        if values_count == 0 {
            bail!("Failed run to ffmpeg to measure loudness values: \n{err_log}");
        }

        if values_count != 10 {
            bail!(
                "ffmpeg returns {values_count} loudness value(s) instead of 10: \n{:#?}",
                values
            );
        }

        if !err_parse.is_empty() {
            bail!("ffmpeg returns strange loudness values: {err_parse}");
        }

        Ok(values)
    }
}
