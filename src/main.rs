mod cli;

use anyhow::{bail, Context, Result};
use clap::Parser;
use cli::{Cli, Command};
use lazy_static::lazy_static;
use props_rs::{parse, Property};
use regex::Regex;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command as osCommand, Stdio};

#[derive(Debug, Default)]
struct EbuLoudnessValues {
    input_i: f64,
    input_tp: f64,
    input_lra: f64,
    input_thresh: f64,
    output_i: f64,
    output_tp: f64,
    output_lra: f64,
    output_thresh: f64,
    normalization_type: String,
    target_offset: f64,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // get input file information
    let input_file_info =
        file_info(&cli.input_file).with_context(|| "Failed to get input file information")?;

    match cli.command {
        Command::Ebu {
            target_level,
            loudness_range_target,
            true_peak,
            offset,
            ffmpeg_args,
        } => normalize_ebu(
            &cli.input_file,
            input_file_info,
            target_level,
            loudness_range_target,
            true_peak,
            offset,
            &ffmpeg_args,
        ),
    }
}

// ffprobe -i 10_seconds.ac3 -show_entries format=duration,bit_rate
// ffprobe -i 10_seconds.ac3 -show_streams -select_streams a:0
// ffprobe -i 10_seconds.ac3 -show_entries format=size,duration:stream=codec_long_name,codec_name,bit_rate,channel_layout -v quiet -of default=noprint_wrappers=1
// ffprobe -i 10_seconds.ac3 -show_streams -select_streams a:0 -v quiet -of default=noprint_wrappers=1
fn file_info(file: &Path) -> Result<Vec<Property>> {
    /* Check input file exist or not */
    {
        OpenOptions::new()
            .read(true)
            .open(file)
            .with_context(|| format!("Can't open input file: {}", file.display()))?;
    }

    let output = osCommand::new("ffprobe")
        .arg("-i")
        .arg(file)
        .arg("-v")
        .arg("quiet")
        .arg("-of")
        .arg("default=noprint_wrappers=1")
        .arg("-show_streams")
        .arg("-select_streams")
        .arg("a:0")
        .output()
        .with_context(|| "Failed to run ffprobe for input file")?;

    if !output.status.success() {
        bail!(
            "Failed to run ffprobe for input file, {}, error: \n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )
    }

    return match parse(&output.stdout) {
        Err(error) => bail!("Failed to parse ffprobe output: {error}"),
        Ok(data) => Ok(data),
    };
}

fn normalize_ebu(
    file: &Path,
    input_file_info: Vec<Property>,
    target_level: f64,
    loudness_range_target: f64,
    true_peak: f64,
    offset: f64,
    ffmpeg_args: &[String],
) -> Result<()> {
    let ebu_values = normalize_ebu_pass1(
        file,
        &input_file_info,
        target_level,
        loudness_range_target,
        true_peak,
        offset,
        ffmpeg_args,
    )
    .with_context(|| "Failed to run pass 1 to measure loudness values")?;

    println!("{:#?}", ebu_values);

    Ok(())
}

// ffmpeg -progress - -nostats -nostdin -y -i 10_seconds.ac3 -filter_complex "[0:0]loudnorm=i=-23.0:lra=7.0:tp=-2.0:offset=0.0:print_format=json" -vn -sn -f null NUL
fn normalize_ebu_pass1(
    file: &Path,
    input_file_info: &[Property],
    target_level: f64,
    loudness_range_target: f64,
    true_peak: f64,
    offset: f64,
    ffmpeg_args: &[String],
) -> Result<EbuLoudnessValues> {
    let mut cmd = osCommand::new("ffmpeg");

    cmd.arg("-progress")
        .arg("-")
        .arg("-nostats")
        .arg("-nostdin")
        .arg("-hide_banner")
        .arg("-y")
        .arg("-i")
        .arg(file)
        .arg("-filter_complex")
        .arg(format!(
            "loudnorm=i={target_level}:lra={loudness_range_target}:tp={true_peak}:offset={offset}:print_format=json"
        ));

    ffmpeg_args.iter().for_each(|arg| {
        cmd.arg(arg);
    });

    cmd.arg("-vn").arg("-sn").arg("-f").arg("null").arg("-");

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().with_context(|| "Failed to run ffmpeg tool")?;

    let stdout = child
        .stdout
        .take()
        .with_context(|| "Failed to open ffmpeg stdout")?;
    let stderr = child
        .stderr
        .take()
        .with_context(|| "Failed to open ffmpeg stderr")?;

    let stdout_reader = BufReader::new(stdout);
    stdout_reader
        .lines()
        .filter_map(|line| line.ok())
        .filter(|line| line.starts_with("out_time="))
        .for_each(|line| {
            // TODO: add progress reporting
            println!("{line}");
        });

    let mut is_json = false;
    let mut err_log = String::new();
    let mut err_parse = String::new();
    let mut values = EbuLoudnessValues {
        ..Default::default()
    };
    let mut values_count = 0;

    lazy_static! {
        static ref RE_VALUES: Regex = Regex::new(r#"^\s*"(\S+)"\s*:\s*"(\S+)",?\s*$"#).unwrap();
    }

    let stderr_reader = BufReader::new(stderr);
    stderr_reader
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
                    values.normalization_type = value_str;
                    values_count += 1;
                }
                _ => {
                    match value_str.parse::<f64>() {
                        Ok(value) => {
                            match field {
                                "input_i" => values.input_i = value,
                                "input_tp" => values.input_tp = value,
                                "input_lra" => values.input_lra = value,
                                "input_thresh" => values.input_thresh = value,
                                "output_i" => values.output_i = value,
                                "output_tp" => values.output_tp = value,
                                "output_lra" => values.output_lra = value,
                                "output_thresh" => values.output_thresh = value,
                                "target_offset" => values.target_offset = value,
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
