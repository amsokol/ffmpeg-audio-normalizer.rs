use anyhow::{bail, Context, Result};
use props_rs::{parse, Property};
use std::fs::OpenOptions;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

pub fn file_info(file: &Path) -> Result<Vec<Property>> {
    /* Check input file exist or not */
    {
        OpenOptions::new()
            .read(true)
            .open(file)
            .with_context(|| format!("Can't open input file: {}", file.display()))?;
    }

    let output = Command::new("ffprobe")
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
        .with_context(|| "Failed to run ffprobe")?;

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

pub fn file_property(info: &[Property], name: &str) -> Option<String> {
    for prop in info.iter() {
        if name.eq(&prop.key) {
            return Some(prop.value.clone());
        }
    }

    None
}

pub fn file_codec_name(info: &[Property]) -> String {
    file_property(info, "codec_name").unwrap_or_else(|| "N/A".to_string())
}

pub fn file_channels(info: &[Property]) -> String {
    file_property(info, "channels").unwrap_or_else(|| "N/A".to_string())
}

pub fn file_channel_layout(info: &[Property]) -> String {
    file_property(info, "channel_layout").unwrap_or_else(|| "N/A".to_string())
}

pub fn parse_duration(val: &str) -> Option<Duration> {
    match val.parse::<f64>() {
        Ok(value) => Some(Duration::from_millis((value * 1000.0).trunc() as u64)),
        Err(_) => None,
    }
}

pub fn file_duration(info: &[Property]) -> Option<Duration> {
    match file_property(info, "duration") {
        Some(duration) => parse_duration(&duration),
        None => None,
    }
}

pub fn file_bit_rate(info: &[Property]) -> String {
    match file_property(info, "bit_rate") {
        Some(val) => match val.parse::<i64>() {
            Ok(bit_rate) => format!("{} kb/s", bit_rate / 1000),
            Err(_) => "N/A".to_string(),
        },
        None => "N/A".to_string(),
    }
}

pub fn file_sample_rate(info: &[Property]) -> String {
    match file_property(info, "sample_rate") {
        Some(val) => match val.parse::<f64>() {
            Ok(sample_rate) => format!("{:.1} kHz", sample_rate / 1000.0),
            Err(_) => "N/A".to_string(),
        },
        None => "N/A".to_string(),
    }
}
