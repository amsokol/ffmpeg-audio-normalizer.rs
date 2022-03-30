use anyhow::{bail, Context, Result};
use props_rs::{parse, Property};
use std::env::consts::OS;
use std::env::current_dir;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

fn ffprobe_file_path() -> PathBuf {
    let mut path = current_dir().unwrap_or_default();
    let ffprobe = match OS {
        "windows" => "ffprobe.exe",
        _ => "ffprobe",
    };

    path.push(ffprobe);

    if !Path::new(&path).exists() {
        path.clear();
        path.push(ffprobe);
    }

    path
}

pub fn file_info(file: &Path) -> Result<Vec<Property>> {
    /* Check input file exist or not */
    {
        OpenOptions::new()
            .read(true)
            .open(file)
            .with_context(|| format!("Can't open input file: {}", file.display()))?;
    }

    let output = Command::new(ffprobe_file_path())
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

fn file_property(info: &[Property], name: &str) -> Option<String> {
    for prop in info.iter() {
        if name.eq(&prop.key) {
            return Some(prop.value.clone());
        }
    }

    None
}

pub fn file_codec_name(info: &[Property]) -> Option<String> {
    file_property(info, "codec_name")
}

pub fn file_channels(info: &[Property]) -> Option<String> {
    file_property(info, "channels")
}

pub fn file_channel_layout(info: &[Property]) -> Option<String> {
    file_property(info, "channel_layout")
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

pub fn file_bit_rate(info: &[Property]) -> Option<i64> {
    if let Some(val) = file_property(info, "bit_rate") {
        if let Ok(bit_rate) = val.parse::<i64>() {
            return Some(bit_rate);
        };
    }

    None
}

pub fn file_bit_rate_txt(info: &[Property]) -> String {
    match file_bit_rate(info) {
        Some(bit_rate) => format!("{} kb/s", bit_rate / 1000),
        None => "N/A".to_string(),
    }
}

pub fn file_sample_rate_txt(info: &[Property]) -> String {
    match file_property(info, "sample_rate") {
        Some(val) => match val.parse::<f64>() {
            Ok(sample_rate) => format!("{:.1} kHz", sample_rate / 1000.0),
            Err(_) => "N/A".to_string(),
        },
        None => "N/A".to_string(),
    }
}
