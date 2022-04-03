use crate::ffmpeg::{EbuLoudnessValues, FFmpeg};
use crate::ffprobe::{FFprobe, FileInfo};
use anyhow::{anyhow, Context, Ok, Result};
use hhmmss::Hhmmss;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const NA: &str = "N/A";

pub struct EbuR128NormalizationArgs<'a> {
    pub verbose: bool,
    pub input_file: &'a Path,
    pub output_file: &'a Path,
    pub target_level: f64,
    pub loudness_range_target: f64,
    pub true_peak: f64,
    pub offset: f64,
    pub ffmpeg_args: &'a [String],
}

struct EbuR128NormalizationCommonArgs<'a> {
    verbose: bool,
    input_file: &'a Path,
    input_file_duration: Option<Duration>,
    input_file_bit_rate: Option<i64>,
    input_file_codec_name: Option<String>,
    target_level: f64,
    loudness_range_target: f64,
    true_peak: f64,
    offset: f64,
    ffmpeg_args: &'a [String],
}

struct EbuR128NormalizationPass1Args<'a> {
    common_args: &'a EbuR128NormalizationCommonArgs<'a>,
    input_file_info: FileInfo,
}

struct EbuR128NormalizationPass2Args<'a> {
    common_args: &'a EbuR128NormalizationCommonArgs<'a>,
    measured_i: f64,
    measured_lra: f64,
    measured_tp: f64,
    measured_thresh: f64,
    target_offset: f64,
    output_file: &'a Path,
}

pub fn normalize_ebu_r128(args: EbuR128NormalizationArgs) -> Result<()> {
    // get input file information
    let input_file_info =
        FFprobe::info(args.input_file).with_context(|| "Failed to get input file information")?;

    let common_args = EbuR128NormalizationCommonArgs {
        verbose: args.verbose,
        input_file: args.input_file,
        input_file_duration: input_file_info.duration(),
        input_file_bit_rate: input_file_info.bit_rate(),
        input_file_codec_name: input_file_info.codec_name(),
        target_level: args.target_level,
        loudness_range_target: args.loudness_range_target,
        true_peak: args.true_peak,
        offset: args.offset,
        ffmpeg_args: args.ffmpeg_args,
    };

    let values = pass1(EbuR128NormalizationPass1Args {
        common_args: &common_args,
        input_file_info,
    })
    .with_context(|| "Failed to run pass 1 to measure loudness values")?;

    pass2(EbuR128NormalizationPass2Args {
        common_args: &common_args,
        measured_i: values
            .input_i
            .ok_or_else(|| anyhow!("EBU normalization pass 1 does not return \"input_i\""))?,
        measured_lra: values
            .input_lra
            .ok_or_else(|| anyhow!("EBU normalization pass 1 does not return \"input_lra\""))?,
        measured_tp: values
            .input_tp
            .ok_or_else(|| anyhow!("EBU normalization pass 1 does not return \"input_tp\""))?,
        measured_thresh: values
            .input_thresh
            .ok_or_else(|| anyhow!("EBU normalization pass 1 does not return \"input_thresh\""))?,
        target_offset: values
            .target_offset
            .ok_or_else(|| anyhow!("EBU normalization pass 1 does not return \"target_offset\""))?,
        output_file: args.output_file,
    })
    .with_context(|| "Failed to run pass 2 to measure loudness values")?;

    Ok(())
}

fn add_common_args(cmd: &mut Command, args: &EbuR128NormalizationCommonArgs) {
    // set bit rate
    if args.input_file_bit_rate.is_some() {
        cmd.arg("-b:a")
            .arg(format!("{}", args.input_file_bit_rate.unwrap()));
    }

    // set codec name
    if args.input_file_codec_name.is_some() {
        cmd.arg("-c:a")
            .arg(args.input_file_codec_name.as_ref().unwrap());
    }

    // custom args
    args.ffmpeg_args.iter().for_each(|arg| {
        cmd.arg(arg);
    });
}

fn pass1(args: EbuR128NormalizationPass1Args) -> Result<EbuLoudnessValues> {
    // show input file information
    println!(
        "Input audio file: \n {}",
        args.common_args.input_file.display()
    );
    println!(
        " Codec: {}, Channels: {}, Channel-layout: {}, Duration: {}, Bit-rate: {}, Sample-rate: {}",
        args.common_args
            .input_file_codec_name
            .clone()
            .unwrap_or_else(|| NA.to_string()),
        args.input_file_info
            .channels()
            .unwrap_or_else(|| NA.to_string()),
        args.input_file_info
            .channel_layout()
            .unwrap_or_else(|| NA.to_string()),
        args.common_args
            .input_file_duration
            .map_or(NA.to_string(), |v| v.hhmmss()),
        args.input_file_info.bit_rate_as_txt(),
        args.input_file_info.sample_rate(),
    );

    let mut ffmpeg = FFmpeg::new(args.common_args.input_file);

    let cmd = ffmpeg.cmd();

    cmd.arg("-filter").arg(format!(
        // "[0:0]loudnorm=i={}:lra={}:tp={}:offset={}:print_format=json",
        "loudnorm=i={}:lra={}:tp={}:offset={}:print_format=json",
        args.common_args.target_level,
        args.common_args.loudness_range_target,
        args.common_args.true_peak,
        args.common_args.offset
    ));

    add_common_args(cmd, args.common_args);

    cmd
        // As an input option, blocks all video streams. As an output option, disables video recording.
        // .arg("-vn")
        // As an input option, blocks all subtitle streams. As an output option, disables subtitle recording.
        // .arg("-sn")
        // output file is NULL
        .arg("-f")
        .arg("null")
        .arg("-");

    ffmpeg
        .exec(
            "[1/2] Processing audio file to measure loudness values:",
            args.common_args.verbose,
            args.common_args.input_file_duration,
        )
        .with_context(|| "Failed to processing audio file to measure loudness values")
}

fn pass2(args: EbuR128NormalizationPass2Args) -> Result<EbuLoudnessValues> {
    let mut ffmpeg = FFmpeg::new(args.common_args.input_file);

    let cmd = ffmpeg.cmd();

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

    cmd
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

    add_common_args(cmd, args.common_args);

    cmd
        // overwrite output files without asking
        .arg("-y")
        .arg(args.output_file);

    let values = ffmpeg
        .exec(
            "[2/2] Normalizing audio file:",
            args.common_args.verbose,
            args.common_args.input_file_duration,
        )
        .with_context(|| "Failed to normalizing audio file")?;

    if args.common_args.verbose {
        println!(
            "  input_i={}",
            values.input_i.map_or(NA.to_string(), |v| v.to_string())
        );
        println!(
            "  input_tp={}",
            values.input_tp.map_or(NA.to_string(), |v| v.to_string())
        );
        println!(
            "  input_lra={}",
            values.input_lra.map_or(NA.to_string(), |v| v.to_string())
        );
        println!(
            "  input_thresh={}",
            values
                .input_thresh
                .map_or(NA.to_string(), |v| v.to_string())
        );
        println!(
            "  output_i={}",
            values.output_i.map_or(NA.to_string(), |v| v.to_string())
        );
        println!(
            "  output_tp={}",
            values.output_tp.map_or(NA.to_string(), |v| v.to_string())
        );
        println!(
            "  output_lra={}",
            values.output_lra.map_or(NA.to_string(), |v| v.to_string())
        );
        println!(
            "  output_thresh={}",
            values
                .output_thresh
                .map_or(NA.to_string(), |v| v.to_string())
        );
        println!(
            "  normalization_type={}",
            values
                .normalization_type
                .clone()
                .unwrap_or_else(|| NA.to_string())
        );
        println!(
            "  target_offset={}",
            values
                .target_offset
                .map_or(NA.to_string(), |v| v.to_string())
        );
    }

    let output_file_info =
        FFprobe::info(args.output_file).with_context(|| "Failed to get output file information")?;

    println!("Output audio file: \n {}", args.output_file.display());
    println!(
        " Codec: {}, Channels: {}, Channel-layout: {}, Duration: {}, Bit-rate: {}, Sample-rate: {}",
        output_file_info
            .codec_name()
            .unwrap_or_else(|| NA.to_string()),
        output_file_info
            .channels()
            .unwrap_or_else(|| NA.to_string()),
        output_file_info
            .channel_layout()
            .unwrap_or_else(|| NA.to_string()),
        output_file_info
            .duration()
            .map_or(NA.to_string(), |v| v.hhmmss()),
        output_file_info.bit_rate_as_txt(),
        output_file_info.sample_rate(),
    );

    Ok(values)
}
