use anyhow::{bail, Result};
use lazy_static::lazy_static;
use regex::Regex;
use std::env::consts::OS;
use std::env::current_dir;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

pub enum Progress {
    OutTime(u64),
    End,
}

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

pub struct FFmpeg {}

impl FFmpeg {
    pub fn ffmpeg_path() -> PathBuf {
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

        path
    }

    pub fn progress<R: Read, F>(reader: BufReader<R>, f: F)
    where
        F: Fn(Progress),
    {
        reader
            .lines()
            .filter_map(|line| line.ok())
            .for_each(|line| {
                if line == "progress=end" {
                    f(Progress::End);
                } else if let Some(ms) = RE_DURATION
                    .captures(line.as_str())
                    .and_then(|m| m.get(1).map(|m| m.as_str()))
                    .and_then(|ms| ms.parse::<u64>().ok())
                {
                    f(Progress::OutTime(ms))
                }
            });
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
