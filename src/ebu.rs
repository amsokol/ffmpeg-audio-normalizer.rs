use crate::file::{
    file_bit_rate, file_channel_layout, file_channels, file_codec_name, file_duration,
    file_sample_rate,
};
use anyhow::{bail, Context, Result};
use hhmmss::Hhmmss;
use lazy_static::lazy_static;
use props_rs::Property;
use regex::Regex;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

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

pub fn normalize_ebu(
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
    let duration = file_duration(input_file_info);
    let duration_txt: String;

    // show input file information
    println!("Input audio file: \n {}", file.display());
    println!(
        " Codec: {}, Channels: {}, Channel-layout: {}, Duration: {}, Bit-rate: {}, Sample-rate: {}",
        file_codec_name(input_file_info),
        file_channels(input_file_info),
        file_channel_layout(input_file_info),
        match duration {
            None => "unknown",
            Some(duration) => {
                duration_txt = duration.hhmmss();
                &duration_txt
            }
        },
        file_bit_rate(input_file_info),
        file_sample_rate(input_file_info),
    );

    let mut cmd = Command::new("ffmpeg");

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
            "[0:0]loudnorm=i={target_level}:lra={loudness_range_target}:tp={true_peak}:offset={offset}:print_format=json"
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
