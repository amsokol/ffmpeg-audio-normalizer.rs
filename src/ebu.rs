use crate::ffmpeg::{get_progress, get_result, EbuLoudnessValues};
use crate::ffprobe::{
    file_bit_rate, file_channel_layout, file_channels, file_codec_name, file_duration,
    file_sample_rate,
};
use anyhow::{bail, Context, Result};
use hhmmss::Hhmmss;
use indicatif::{ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
use props_rs::Property;
use regex::Regex;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

pub fn normalize_ebu(
    file: &Path,
    input_file_info: Vec<Property>,
    target_level: f64,
    loudness_range_target: f64,
    true_peak: f64,
    offset: f64,
    ffmpeg_args: &[String],
) -> Result<()> {
    let ebu_values = normalize_ebu_pass1(
        file,
        &input_file_info,
        target_level,
        loudness_range_target,
        true_peak,
        offset,
        ffmpeg_args,
    )
    .with_context(|| "Failed to run pass 1 to measure loudness values")?;

    println!("{:#?}", ebu_values);

    Ok(())
}

lazy_static! {
    static ref RE_VALUES: Regex = Regex::new(r#"^\s*"(\S+)"\s*:\s*"(\S+)",?\s*$"#).unwrap();
}

fn normalize_ebu_pass1(
    file: &Path,
    input_file_info: &[Property],
    target_level: f64,
    loudness_range_target: f64,
    true_peak: f64,
    offset: f64,
    ffmpeg_args: &[String],
) -> Result<EbuLoudnessValues> {
    let duration = file_duration(input_file_info);
    let duration_txt: String;

    // show input file information
    println!("Input audio file: \n {}", file.display());
    println!(
        " Codec: {}, Channels: {}, Channel-layout: {}, Duration: {}, Bit-rate: {}, Sample-rate: {}",
        file_codec_name(input_file_info),
        file_channels(input_file_info),
        file_channel_layout(input_file_info),
        match duration {
            None => "unknown",
            Some(duration) => {
                duration_txt = duration.hhmmss();
                &duration_txt
            }
        },
        file_bit_rate(input_file_info),
        file_sample_rate(input_file_info),
    );

    let mut cmd = Command::new("ffmpeg");

    cmd.arg("-progress")
        .arg("-")
        .arg("-nostats")
        .arg("-nostdin")
        .arg("-hide_banner")
        .arg("-y")
        .arg("-i")
        .arg(file)
        .arg("-filter_complex")
        .arg(format!(
            "[0:0]loudnorm=i={target_level}:lra={loudness_range_target}:tp={true_peak}:offset={offset}:print_format=json"
        ));

    ffmpeg_args.iter().for_each(|arg| {
        cmd.arg(arg);
    });

    cmd.arg("-vn").arg("-sn").arg("-f").arg("null").arg("-");

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().with_context(|| "Failed to run ffmpeg tool")?;

    if let Some(long) = duration {
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
}
