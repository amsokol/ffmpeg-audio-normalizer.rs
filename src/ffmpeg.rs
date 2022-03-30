use anyhow::{bail, Result};
use lazy_static::lazy_static;
use regex::Regex;
use std::env::consts::OS;
use std::env::current_dir;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Default)]
pub struct EbuLoudnessValues {
    pub input_i: Option<f64>,
    pub input_tp: Option<f64>,
    pub input_lra: Option<f64>,
    pub input_thresh: Option<f64>,
    pub output_i: Option<f64>,
    pub output_tp: Option<f64>,
    pub output_lra: Option<f64>,
    pub output_thresh: Option<f64>,
    pub normalization_type: Option<String>,
    pub target_offset: Option<f64>,
}

lazy_static! {
    static ref RE_DURATION: Regex =
        Regex::new(r#"^\s*out_time\s*=\s*(\d\d):(\d\d):(\d\d).*$"#).unwrap();
    static ref RE_VALUES: Regex = Regex::new(r#"^\s*"(\S+)"\s*:\s*"(\S+)",?\s*$"#).unwrap();
}

pub fn ffmpeg_file_path() -> PathBuf {
    let mut path = current_dir().unwrap_or_default();

    path.push("ffmpeg");
    if OS == "windows" {
        path.push(".exe");
    }

    if !Path::new(&path).exists() {
        path.clear();
        path.push("ffmpeg");
    }

    path
}

fn parse_progress(val: &str) -> Option<Duration> {
    if let Some(m) = RE_DURATION.captures(val) {
        let hh = m.get(1).map_or("", |m| m.as_str());
        let mm = m.get(2).map_or("", |m| m.as_str());
        let ss = m.get(3).map_or("", |m| m.as_str());

        let mut progress_in_seconds: u64 = 0;

        if let Ok(hours) = hh.parse::<u64>() {
            progress_in_seconds += hours * 60 * 60;

            if let Ok(minutes) = mm.parse::<u64>() {
                progress_in_seconds += minutes * 60;

                if let Ok(seconds) = ss.parse::<u64>() {
                    progress_in_seconds += seconds;
                }

                return Some(Duration::from_secs(progress_in_seconds));
            }
        }
    }

    None
}

pub fn get_progress<R: Read, F>(reader: BufReader<R>, f: F)
where
    F: Fn(Duration),
{
    reader
        .lines()
        .filter_map(|line| line.ok())
        .for_each(|line| {
            if let Some(progress) = parse_progress(&line) {
                f(progress);
            }
        });
}

pub fn get_result<R: Read>(reader: BufReader<R>) -> Result<EbuLoudnessValues> {
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
