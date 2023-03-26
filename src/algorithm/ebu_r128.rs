use crate::io::to_stdout;
use crate::tool::ffmpeg::FFmpeg;
use crate::tool::ffprobe::{AudioStream, FFprobe};
use anyhow::{Context, Result};
use lazy_static::lazy_static;
use regex::Regex;
use serde::{de::Error, Deserialize, Deserializer};
use std::{io::BufRead, path::Path};

lazy_static! {
    static ref RE_VALUES: Regex = Regex::new(r#"^\s*"(\S+)"\s*:\s*"(\S+)",?\s*$"#).unwrap();
}

#[derive(Deserialize)]
struct LoudnessValues {
    #[serde(deserialize_with = "f64_from_string")]
    input_i: f64,
    #[serde(deserialize_with = "f64_from_string")]
    input_lra: f64,
    #[serde(deserialize_with = "f64_from_string")]
    input_tp: f64,
    #[serde(deserialize_with = "f64_from_string")]
    input_thresh: f64,
    #[serde(deserialize_with = "f64_from_string")]
    target_offset: f64,
}

fn f64_from_string<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    let s: &str = Deserialize::deserialize(deserializer)?;
    s.parse::<f64>()
        .map_err(|err| D::Error::custom(err.to_string()))
}

pub struct NormalizationArgs<'a> {
    pub verbose: bool,
    pub input_file: &'a Path,
    pub output_file: &'a Path,
    pub overwrite: bool,
    pub target_level: f64,
    pub loudness_range_target: f64,
    pub true_peak: f64,
    pub offset: f64,
    pub ffmpeg_args: &'a [String],
}

struct NormalizationCommonArgs<'a> {
    verbose: bool,
    input_file: &'a Path,
    input_file_info: AudioStream,
    target_level: f64,
    loudness_range_target: f64,
    true_peak: f64,
    offset: f64,
    ffmpeg_args: &'a [String],
}

struct NormalizationPass1Args<'a> {
    common_args: &'a NormalizationCommonArgs<'a>,
}

struct NormalizationPass2Args<'a> {
    common_args: &'a NormalizationCommonArgs<'a>,
    measured_i: f64,
    measured_lra: f64,
    measured_tp: f64,
    measured_thresh: f64,
    target_offset: f64,
    output_file: &'a Path,
    overwrite: bool,
}

pub fn normalize(args: NormalizationArgs) -> Result<()> {
    // get input file information
    let input_file_info =
        FFprobe::info(args.input_file).with_context(|| "Failed to get input file information")?;

    let common_args = NormalizationCommonArgs {
        verbose: args.verbose,
        input_file: args.input_file,
        input_file_info,
        target_level: args.target_level,
        loudness_range_target: args.loudness_range_target,
        true_peak: args.true_peak,
        offset: args.offset,
        ffmpeg_args: args.ffmpeg_args,
    };

    let values = pass1(NormalizationPass1Args {
        common_args: &common_args,
    })
    .with_context(|| "Failed to run pass 1 to measure loudness values")?;

    pass2(NormalizationPass2Args {
        common_args: &common_args,
        measured_i: values.input_i,
        measured_lra: values.input_lra,
        measured_tp: values.input_tp,
        measured_thresh: values.input_thresh,
        target_offset: values.target_offset,
        output_file: args.output_file,
        overwrite: args.overwrite,
    })
    .with_context(|| "Failed to run pass 2 to normalize audio file")?;

    Ok(())
}

fn pass1(args: NormalizationPass1Args) -> Result<LoudnessValues> {
    let mut ffmpeg = FFmpeg::new(args.common_args.input_file);

    ffmpeg.cmd().arg("-filter_complex").arg(format!(
        "loudnorm=i={}:lra={}:tp={}:offset={}:print_format=json",
        args.common_args.target_level,
        args.common_args.loudness_range_target,
        args.common_args.true_peak,
        args.common_args.offset
    ));

    ffmpeg.add_common_args(
        &args.common_args.input_file_info,
        args.common_args.ffmpeg_args,
    );

    ffmpeg.cmd().arg("-f").arg("null").arg("-");

    let reader = ffmpeg
        .exec(
            "[1/2] Processing audio file to measure loudness values:",
            args.common_args.verbose,
            args.common_args.input_file_info.duration,
        )
        .with_context(|| "Failed to processing audio file to measure loudness values")?;

    let mut is_json = false;

    let lines: Vec<String> = reader
        .lines()
        .filter_map(|line| line.ok())
        .filter(|line| match line.as_str() {
            "{" => {
                is_json = true;
                true
            }
            "}" => {
                is_json = false;
                true
            }
            _ => is_json,
        })
        .collect();

    serde_json::from_str(lines.join("\n").as_str())
        .with_context(|| "Failed to parse measure result - invalid JSON")
}

fn pass2(args: NormalizationPass2Args) -> Result<()> {
    let mut ffmpeg = FFmpeg::new(args.common_args.input_file);

    let mut filter = format!(
        "loudnorm=i={}:lra={}:tp={}:offset={}",
        args.common_args.target_level,
        args.common_args.loudness_range_target,
        args.common_args.true_peak,
        args.target_offset
    );

    filter += format!(
        ":measured_i={}:measured_lra={}:measured_tp={}:measured_thresh={}",
        args.measured_i, args.measured_lra, args.measured_tp, args.measured_thresh
    )
    .as_str();

    ffmpeg
        .cmd()
        .arg("-filter_complex")
        .arg(filter + ":linear=true:print_format=json");

    ffmpeg.add_common_args(
        &args.common_args.input_file_info,
        args.common_args.ffmpeg_args,
    );

    if args.overwrite {
        ffmpeg.cmd().arg("-y");
    }
    ffmpeg.cmd().arg(args.output_file);

    let reader = ffmpeg
        .exec(
            "[2/2] EBU R128 Normalizing audio file:",
            args.common_args.verbose,
            args.common_args.input_file_info.duration,
        )
        .with_context(|| "Failed to normalizing audio file")?;

    to_stdout(reader);

    Ok(())
}
