use crate::algorithm::io::to_stdout;
use crate::tool::ffmpeg::FFmpeg;
use crate::tool::ffprobe::{FFprobe, FileInfo};
use anyhow::{bail, Context, Result};
use lazy_static::lazy_static;
use regex::Regex;
use std::fmt::Write as _;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::ChildStderr;

lazy_static! {
    static ref RE_VALUES: Regex =
        Regex::new(r#"^\s*.*\s*Peak\s+level\s+dB\s*:\s*(.+)\s*$"#).unwrap();
}

pub struct NormalizationArgs<'a> {
    pub verbose: bool,
    pub input_file: &'a Path,
    pub output_file: &'a Path,
    pub overwrite: bool,
    pub target_level: f64,
    pub ffmpeg_args: &'a [String],
}

struct NormalizationCommonArgs<'a> {
    verbose: bool,
    input_file: &'a Path,
    input_file_info: FileInfo,
    ffmpeg_args: &'a [String],
}

struct NormalizationPass1Args<'a> {
    common_args: &'a NormalizationCommonArgs<'a>,
}

struct NormalizationPass2Args<'a> {
    common_args: &'a NormalizationCommonArgs<'a>,
    volume_adjustment: f64,
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
        ffmpeg_args: args.ffmpeg_args,
    };

    let value = pass1(NormalizationPass1Args {
        common_args: &common_args,
    })
    .with_context(|| "Failed to run pass 1 to measure loudness values")?;

    pass2(NormalizationPass2Args {
        common_args: &common_args,
        volume_adjustment: args.target_level - value,
        output_file: args.output_file,
        overwrite: args.overwrite,
    })
    .with_context(|| "Failed to run pass 2 to normalize audio file")?;

    Ok(())
}

fn pass1(args: NormalizationPass1Args) -> Result<f64> {
    let mut ffmpeg = FFmpeg::new(args.common_args.input_file);

    ffmpeg
        .cmd()
        .arg("-filter")
        .arg("astats=measure_overall=Peak_level:measure_perchannel=0");

    ffmpeg.add_common_args(
        &args.common_args.input_file_info,
        args.common_args.ffmpeg_args,
    );

    ffmpeg.cmd().arg("-f").arg("null").arg("-");

    let reader = ffmpeg
        .exec(
            "[1/2] Processing audio file to measure loudness values:",
            args.common_args.verbose,
            args.common_args.input_file_info.duration(),
        )
        .with_context(|| "Failed to processing audio file to measure loudness values")?;

    let level =
        result_pass1(reader).with_context(|| "Failed to parse Peak level measure result")?;

    if args.common_args.verbose {
        println!("  Peak level = {}dB", level);
    }

    Ok(level)
}

fn pass2(args: NormalizationPass2Args) -> Result<()> {
    let mut ffmpeg = FFmpeg::new(args.common_args.input_file);

    ffmpeg
        .cmd()
        .arg("-filter")
        .arg(format!("volume={}dB", args.volume_adjustment));

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
            "[2/2] Peak Normalizing audio file:",
            args.common_args.verbose,
            args.common_args.input_file_info.duration(),
        )
        .with_context(|| "Failed to normalizing audio file")?;

    if args.common_args.verbose {
        println!("  Volume adjustment = {}dB", args.volume_adjustment);
    }

    to_stdout(reader);

    Ok(())
}

fn result_pass1(reader: BufReader<ChildStderr>) -> Result<f64> {
    let mut err_log = String::new();
    let mut err_parse = String::new();
    let mut value = 0.0f64;
    let mut values_found = false;

    reader
        .lines()
        .filter_map(|line| line.ok())
        .for_each(|line| {
            if let Some(m) = RE_VALUES.captures(&line).and_then(|caps| caps.get(1)) {
                if let Ok(v) = m.as_str().parse::<f64>() {
                    value = v;
                    values_found = true;
                } else {
                    let _ = writeln!(err_parse, "Failed to parse Peak level value: {}", line);
                }
            } else {
                // log error in case of problems
                err_log += &line;
                err_log += "\n";
            }
        });

    if !values_found {
        if !err_parse.is_empty() {
            bail!("ffmpeg returns strange Peak level value: {err_parse}");
        } else {
            bail!("Failed run to ffmpeg to measure Peak level value: \n{err_log}");
        }
    }

    Ok(value)
}
