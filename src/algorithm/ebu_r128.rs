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
    static ref RE_VALUES: Regex = Regex::new(r#"^\s*"(\S+)"\s*:\s*"(\S+)",?\s*$"#).unwrap();
}

#[derive(Debug, Default)]
struct LoudnessValues {
    input_i: f64,
    input_lra: f64,
    input_tp: f64,
    input_thresh: f64,
    output_i: f64,
    output_lra: f64,
    output_tp: f64,
    output_thresh: f64,
    normalization_type: String,
    target_offset: f64,
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
    input_file_info: FileInfo,
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

    ffmpeg.cmd().arg("-filter").arg(format!(
        // "[0:0]loudnorm=i={}:lra={}:tp={}:offset={}:print_format=json",
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

    ffmpeg
        .cmd()
        // As an input option, blocks all video streams. As an output option, disables video recording.
        // .arg("-vn")
        // As an input option, blocks all subtitle streams. As an output option, disables subtitle recording.
        // .arg("-sn")
        // output file is NULL
        .arg("-f")
        .arg("null")
        .arg("-");

    let reader = ffmpeg
        .exec(
            "[1/2] Processing audio file to measure loudness values:",
            args.common_args.verbose,
            args.common_args.input_file_info.duration(),
        )
        .with_context(|| "Failed to processing audio file to measure loudness values")?;

    result_pass1(reader).with_context(|| "Failed to parse measure result")
}

fn pass2(args: NormalizationPass2Args) -> Result<()> {
    let mut ffmpeg = FFmpeg::new(args.common_args.input_file);

    let mut filter = format!(
        // "[0:0]loudnorm=i={}:lra={}:tp={}:offset={}",
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

    ffmpeg.cmd()
        // .arg("-filter_complex")
        .arg("-filter")
        // .arg(filter + ":linear=true:print_format=json[norm0]")
        .arg(filter + ":linear=true:print_format=json")
        // Set metadata information of the next output file from infile.
        // .arg("-map_metadata")
        // .arg("0")
        // .arg("-map_metadata:s:a:0")
        // .arg("0:s:a:0")
        // Copy chapters from input file with index input_file_index to the next output file.
        // .arg("-map_chapters")
        // .arg("0")
        // Select an encoder (when used before an output file) or a decoder (when used before an input file)
        // for one or more streams. codec is the name of a decoder/encoder or a special value copy (output only)
        // to indicate that the stream is not to be re-encoded.
        // .arg("-c:v")
        // .arg("copy")
        // .arg("-c:s")
        // .arg("copy")
        // Designate one or more input streams as a source for the output file.
        // .arg("-map")
        // .arg("[norm0]")
        ;

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
            args.common_args.input_file_info.duration(),
        )
        .with_context(|| "Failed to normalizing audio file")?;

    to_stdout(reader);

    Ok(())
}

fn result_pass1(reader: BufReader<ChildStderr>) -> Result<LoudnessValues> {
    let mut is_json = false;
    let mut err_log = String::new();
    let mut err_parse = String::new();
    let mut values = LoudnessValues {
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
            if let Some(caps) = RE_VALUES.captures(&line) {
                if let Some(m) = caps.get(1) {
                    let field = m.as_str();
                    if let Some(m) = caps.get(2) {
                        let value_str = m.as_str();

                        match field {
                            "normalization_type" => {
                                values.normalization_type = value_str.to_string();
                                values_count += 1;
                            }
                            _ => {
                                if let Ok(value) = value_str.parse::<f64>() {
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
                                            let _ = writeln!(
                                                err_parse,
                                                "Unknown loudness value: {}",
                                                line
                                            );
                                            return;
                                        }
                                    }
                                    values_count += 1;
                                } else {
                                    let _ = writeln!(err_parse, "Invalid loudness value: {}", line);
                                }
                            }
                        }

                        return;
                    }
                }
            }
            let _ = writeln!(err_parse, "Failed to parse loudness value: {}", line);
        });

    if values_count == 0 {
        if !err_parse.is_empty() {
            bail!("ffmpeg returns strange loudness values: {err_parse}");
        } else {
            bail!("Failed run to ffmpeg to measure loudness values: \n{err_log}");
        }
    }

    if values_count != 10 {
        bail!(
            "ffmpeg returns {values_count} loudness value(s) instead of 10: \n{:#?}",
            values
        );
    }

    Ok(values)
}
