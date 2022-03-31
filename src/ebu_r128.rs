use crate::ffmpeg::{ffmpeg_file_path, get_progress, get_result, EbuLoudnessValues};
use crate::ffprobe::{FFprobe, FileInfo};
use anyhow::{anyhow, Context, Ok, Result};
use hhmmss::Hhmmss;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::BufReader;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

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

pub struct EbuR128NormalizationPass1Args<'a> {
    pub verbose: bool,
    pub input_file: &'a Path,
    pub input_file_duration: Option<Duration>,
    pub input_file_bit_rate: Option<i64>,
    pub input_file_codec_name: Option<String>,
    pub input_file_info: FileInfo,
    pub target_level: f64,
    pub loudness_range_target: f64,
    pub true_peak: f64,
    pub offset: f64,
    pub ffmpeg_args: &'a [String],
}

pub struct EbuR128NormalizationPass2Args<'a> {
    pub verbose: bool,
    pub input_file: &'a Path,
    pub input_file_duration: Option<Duration>,
    pub input_file_bit_rate: Option<i64>,
    pub input_file_codec_name: Option<String>,
    pub output_file: &'a Path,
    pub target_level: f64,
    pub loudness_range_target: f64,
    pub true_peak: f64,
    pub offset: f64,
    pub measured_i: f64,
    pub measured_lra: f64,
    pub measured_tp: f64,
    pub measured_thresh: f64,
    pub ffmpeg_args: &'a [String],
}

pub fn normalize_ebu_r128(args: EbuR128NormalizationArgs) -> Result<()> {
    // get input file information
    let input_file_info =
        FFprobe::info(args.input_file).with_context(|| "Failed to get input file information")?;
    let duration = input_file_info.duration();
    let input_file_bit_rate = input_file_info.bit_rate();
    let input_file_codec_name = input_file_info.codec_name();

    let values = pass1(EbuR128NormalizationPass1Args {
        verbose: args.verbose,
        input_file: args.input_file,
        input_file_duration: duration,
        input_file_bit_rate,
        input_file_codec_name: input_file_codec_name.clone(),
        input_file_info,
        target_level: args.target_level,
        loudness_range_target: args.loudness_range_target,
        true_peak: args.true_peak,
        offset: args.offset,
        ffmpeg_args: args.ffmpeg_args,
    })
    .with_context(|| "Failed to run pass 1 to measure loudness values")?;

    pass2(EbuR128NormalizationPass2Args {
        verbose: args.verbose,
        input_file: args.input_file,
        input_file_duration: duration,
        input_file_bit_rate,
        input_file_codec_name,
        output_file: args.output_file,
        target_level: args.target_level,
        loudness_range_target: args.loudness_range_target,
        true_peak: args.true_peak,
        offset: values
            .target_offset
            .ok_or_else(|| anyhow!("EBU normalization pass 1 does not return \"target_offset\""))?,
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
        ffmpeg_args: args.ffmpeg_args,
    })
    .with_context(|| "Failed to run pass 2 to measure loudness values")?;

    Ok(())
}

fn dump_command_args(cmd: &Command) {
    println!("Running ffmpeg with the following arguments:");
    print!("[ ");
    cmd.get_args()
        .for_each(|arg| print!("{} ", arg.to_str().unwrap_or_default()));
    println!("]");
}

fn pass1(args: EbuR128NormalizationPass1Args) -> Result<EbuLoudnessValues> {
    // show input file information
    println!("Input audio file: \n {}", args.input_file.display());
    println!(
        " Codec: {}, Channels: {}, Channel-layout: {}, Duration: {}, Bit-rate: {}, Sample-rate: {}",
        args.input_file_info
            .codec_name()
            .unwrap_or_else(|| "N/A".to_string()),
        args.input_file_info
            .channels()
            .unwrap_or_else(|| "N/A".to_string()),
        args.input_file_info
            .channel_layout()
            .unwrap_or_else(|| "N/A".to_string()),
        args.input_file_duration
            .map_or("N/A".to_string(), |v| v.hhmmss()),
        args.input_file_info.bit_rate_as_txt(),
        args.input_file_info.sample_rate(),
    );

    let mut cmd = Command::new(ffmpeg_file_path());

    cmd.arg("-progress")
        .arg("-")
        .arg("-nostats")
        .arg("-nostdin")
        .arg("-y")
        .arg("-hide_banner")
        .arg("-i")
        .arg(args.input_file)
        .arg("-filter_complex")
        .arg(format!(
            "[0:0]loudnorm=i={}:lra={}:tp={}:offset={}:print_format=json",
            args.target_level, args.loudness_range_target, args.true_peak, args.offset
        ));

    // set bit rate
    if args.input_file_bit_rate.is_some() {
        cmd.arg("-b:a")
            .arg(format!("{}", args.input_file_bit_rate.unwrap()));
    }

    // set codec name
    if args.input_file_codec_name.is_some() {
        cmd.arg("-c:a").arg(args.input_file_codec_name.unwrap());
    }

    args.ffmpeg_args.iter().for_each(|arg| {
        cmd.arg(arg);
    });

    cmd.arg("-vn").arg("-sn").arg("-f").arg("null").arg("-");

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().with_context(|| "Failed to run ffmpeg tool")?;

    if let Some(long) = args.input_file_duration {
        println!("[1/2] Processing audio file to measure loudness values:");

        if args.verbose {
            dump_command_args(&cmd);
        }

        let bar = ProgressBar::new(long.as_secs() + 1);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:50.cyan/cyan} {percent}% (estimated: {eta})"),
        );

        get_progress(
            BufReader::new(
                child
                    .stdout
                    .take()
                    .with_context(|| "Failed to open ffmpeg stdout")?,
            ),
            |progress| bar.set_position(progress.as_secs()),
        );

        bar.finish_and_clear();
    } else {
        println!("[1/2] Processing audio file to measure loudness values...");
        if args.verbose {
            dump_command_args(&cmd);
        }
    }

    let values = get_result(BufReader::new(
        child
            .stderr
            .take()
            .with_context(|| "Failed to open ffmpeg stderr")?,
    ))
    .with_context(|| "Failed to get results of pass 2 normalization")?;

    println!("Done.");

    Ok(values)
}

fn pass2(args: EbuR128NormalizationPass2Args) -> Result<EbuLoudnessValues> {
    let mut cmd = Command::new("ffmpeg");

    let mut filter = format!(
        "[0:0]loudnorm=i={}:lra={}:tp={}:offset={}",
        args.target_level, args.loudness_range_target, args.true_peak, args.offset
    );

    filter += format!(
        ":measured_i={}:measured_lra={}:measured_tp={}:measured_thresh={}",
        args.measured_i, args.measured_lra, args.measured_tp, args.measured_thresh
    )
    .as_str();

    cmd.arg("-progress")
        .arg("-")
        .arg("-nostats")
        .arg("-y")
        .arg("-nostdin")
        .arg("-hide_banner")
        .arg("-i")
        .arg(args.input_file)
        .arg("-filter_complex")
        .arg(filter + ":linear=true:print_format=json[norm0]")
        .arg("-map_metadata")
        .arg("0")
        .arg("-map_metadata:s:a:0")
        .arg("0:s:a:0")
        .arg("-map_chapters")
        .arg("0")
        .arg("-c:v")
        .arg("copy")
        .arg("-map")
        .arg("[norm0]");

    // set bit rate
    if args.input_file_bit_rate.is_some() {
        cmd.arg("-b:a")
            .arg(format!("{}", args.input_file_bit_rate.unwrap()));
    }

    // set codec name
    if args.input_file_codec_name.is_some() {
        cmd.arg("-c:a").arg(args.input_file_codec_name.unwrap());
    }

    args.ffmpeg_args.iter().for_each(|arg| {
        cmd.arg(arg);
    });

    cmd.arg("-c:s").arg("copy").arg(args.output_file);

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().with_context(|| "Failed to run ffmpeg tool")?;

    if let Some(long) = args.input_file_duration {
        println!("[2/2] Normalizing audio file:");

        if args.verbose {
            dump_command_args(&cmd);
        }

        let bar = ProgressBar::new(long.as_secs() + 1);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:50.cyan/cyan} {percent}% (estimated: {eta})"),
        );

        get_progress(
            BufReader::new(
                child
                    .stdout
                    .take()
                    .with_context(|| "Failed to open ffmpeg stdout")?,
            ),
            |progress| bar.set_position(progress.as_secs()),
        );

        bar.finish_and_clear();
    } else {
        println!("[2/2] Normalizing audio file...");

        if args.verbose {
            dump_command_args(&cmd);
        }
    }

    let values = get_result(BufReader::new(
        child
            .stderr
            .take()
            .with_context(|| "Failed to open ffmpeg stderr")?,
    ))
    .with_context(|| "Failed to get results of pass 2 normalization")?;

    println!("Done.");

    if args.verbose {
        println!(
            "  input_i={}",
            values.input_i.map_or("N/A".to_string(), |v| v.to_string())
        );
        println!(
            "  input_tp={}",
            values.input_tp.map_or("N/A".to_string(), |v| v.to_string())
        );
        println!(
            "  input_lra={}",
            values
                .input_lra
                .map_or("N/A".to_string(), |v| v.to_string())
        );
        println!(
            "  input_thresh={}",
            values
                .input_thresh
                .map_or("N/A".to_string(), |v| v.to_string())
        );
        println!(
            "  output_i={}",
            values.output_i.map_or("N/A".to_string(), |v| v.to_string())
        );
        println!(
            "  output_tp={}",
            values
                .output_tp
                .map_or("N/A".to_string(), |v| v.to_string())
        );
        println!(
            "  output_lra={}",
            values
                .output_lra
                .map_or("N/A".to_string(), |v| v.to_string())
        );
        println!(
            "  output_thresh={}",
            values
                .output_thresh
                .map_or("N/A".to_string(), |v| v.to_string())
        );
        println!(
            "  normalization_type={}",
            values
                .normalization_type
                .clone()
                .unwrap_or_else(|| "N/A".to_string())
        );
        println!(
            "  target_offset={}",
            values
                .target_offset
                .map_or("N/A".to_string(), |v| v.to_string())
        );
    }

    let output_file_info =
        FFprobe::info(args.output_file).with_context(|| "Failed to get output file information")?;

    println!("Output audio file: \n {}", args.output_file.display());
    println!(
        " Codec: {}, Channels: {}, Channel-layout: {}, Duration: {}, Bit-rate: {}, Sample-rate: {}",
        output_file_info
            .codec_name()
            .unwrap_or_else(|| "N/A".to_string()),
        output_file_info
            .channels()
            .unwrap_or_else(|| "N/A".to_string()),
        output_file_info
            .channel_layout()
            .unwrap_or_else(|| "N/A".to_string()),
        output_file_info
            .duration()
            .map_or("N/A".to_string(), |v| v.hhmmss()),
        output_file_info.bit_rate_as_txt(),
        output_file_info.sample_rate(),
    );

    Ok(values)
}
