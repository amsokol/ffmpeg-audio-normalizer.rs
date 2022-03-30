use crate::ffmpeg::{ffmpeg_file_path, get_progress, get_result, EbuLoudnessValues};
use crate::ffprobe::{
    file_bit_rate, file_bit_rate_txt, file_channel_layout, file_channels, file_codec_name,
    file_duration, file_info, file_sample_rate_txt,
};
use anyhow::{anyhow, Context, Ok, Result};
use hhmmss::Hhmmss;
use indicatif::{ProgressBar, ProgressStyle};
use props_rs::Property;
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
    pub input_file_info: &'a [Property],
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
        file_info(args.input_file).with_context(|| "Failed to get input file information")?;
    let duration = file_duration(&input_file_info);
    let input_file_bit_rate = file_bit_rate(&input_file_info);
    let input_file_codec_name = file_codec_name(&input_file_info);

    let values = pass1(EbuR128NormalizationPass1Args {
        verbose: args.verbose,
        input_file: args.input_file,
        input_file_duration: duration,
        input_file_bit_rate,
        input_file_codec_name: input_file_codec_name.clone(),
        input_file_info: &input_file_info,
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

fn pass1(args: EbuR128NormalizationPass1Args) -> Result<EbuLoudnessValues> {
    // show input file information
    println!("Input audio file: \n {}", args.input_file.display());
    println!(
        " Codec: {}, Channels: {}, Channel-layout: {}, Duration: {}, Bit-rate: {}, Sample-rate: {}",
        file_codec_name(args.input_file_info).unwrap_or_else(|| "N/A".to_string()),
        file_channels(args.input_file_info).unwrap_or_else(|| "N/A".to_string()),
        file_channel_layout(args.input_file_info).unwrap_or_else(|| "N/A".to_string()),
        match args.input_file_duration {
            None => "unknown".to_string(),
            Some(duration) => duration.hhmmss(),
        },
        file_bit_rate_txt(args.input_file_info),
        file_sample_rate_txt(args.input_file_info),
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

    if args.verbose {
        println!("Running ffmpeg with the following arguments for pass 1:");
        print!("[ ");
        cmd.get_args()
            .for_each(|arg| print!("{} ", arg.to_str().unwrap_or_default()));
        println!("]");
    }

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().with_context(|| "Failed to run ffmpeg tool")?;

    if let Some(long) = args.input_file_duration {
        println!("Processing audio file to measure loudness values:");

        let bar = ProgressBar::new(long.as_secs());
        bar.set_style(
            ProgressStyle::default_bar()
                .template("{bar:50.cyan/white} {percent}% (estimated: {eta})"),
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

        bar.finish();
    } else {
        println!("Processing audio file to measure loudness values...");
    }

    get_result(BufReader::new(
        child
            .stderr
            .take()
            .with_context(|| "Failed to open ffmpeg stderr")?,
    ))
    .with_context(|| "Failed to get results of pass 2 normalization")
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

    if args.verbose {
        println!("Running ffmpeg with the following arguments for pass 2:");
        print!("[ ");
        cmd.get_args()
            .for_each(|arg| print!("{} ", arg.to_str().unwrap_or_default()));
        println!("]");
    }

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().with_context(|| "Failed to run ffmpeg tool")?;

    if let Some(long) = args.input_file_duration {
        println!("Normalizing audio file:");

        let bar = ProgressBar::new(long.as_secs());
        bar.set_style(
            ProgressStyle::default_bar()
                .template("{bar:50.cyan/white} {percent}% (estimated: {eta})"),
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

        bar.finish();
    } else {
        println!("Normalizing audio file...");
    }

    let values = get_result(BufReader::new(
        child
            .stderr
            .take()
            .with_context(|| "Failed to open ffmpeg stderr")?,
    ))
    .with_context(|| "Failed to get results of pass 2 normalization")?;

    let output_file_info =
        file_info(args.output_file).with_context(|| "Failed to get output file information")?;

    println!("Output audio file: \n {}", args.output_file.display());
    println!(
        " Codec: {}, Channels: {}, Channel-layout: {}, Duration: {}, Bit-rate: {}, Sample-rate: {}",
        file_codec_name(&output_file_info).unwrap_or_else(|| "N/A".to_string()),
        file_channels(&output_file_info).unwrap_or_else(|| "N/A".to_string()),
        file_channel_layout(&output_file_info).unwrap_or_else(|| "N/A".to_string()),
        match file_duration(&output_file_info) {
            None => "unknown".to_string(),
            Some(duration) => duration.hhmmss(),
        },
        file_bit_rate_txt(&output_file_info),
        file_sample_rate_txt(&output_file_info),
    );

    Ok(values)
}
