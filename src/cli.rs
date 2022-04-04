use clap::{crate_authors, crate_description, crate_name, crate_version, AppSettings, Parser};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[clap(name = crate_name!())]
#[clap(author = crate_authors!("\n"))]
#[clap(version = crate_version!())]
#[clap(about = crate_description!(), long_about = None)]
#[clap(allow_negative_numbers = true)]
#[clap(global_setting(AppSettings::DeriveDisplayOrder))]
pub struct Cli {
    /// Verbose output
    #[clap(long)]
    pub verbose: bool,

    /// Input audio file
    #[clap(long, short, value_name = "INPUT_FILE", parse(from_os_str))]
    pub input_file: PathBuf,

    /// Output audio file after normalization
    #[clap(long, short, value_name = "OUTPUT_FILE", parse(from_os_str))]
    pub output_file: PathBuf,

    /// Force overwrite existing output file
    #[clap(long)]
    pub overwrite: bool,

    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Parser, Debug)]
pub enum Command {
    /// EBU normalization performs two passes and normalizes according to EBU R128.
    Ebu {
        /// Normalization target level in dB/LUFS.
        /// It corresponds to Integrated Loudness Target in LUFS.
        /// The range is [-70.0 .. -5.0].
        #[clap(
            long,
            default_value = "-23.0",
            allow_hyphen_values = true,
            validator = ebu_target_level_validator
        )]
        target_level: f64,

        /// Loudness Range Target in LUFS.
        /// Range is [+1.0 .. +20.0].
        #[clap(
            long,
            default_value = "7.0",
            allow_hyphen_values = true,
            validator=ebu_loudness_range_target_validator
        )]
        loudness_range_target: f64,

        /// Maximum True Peak in dBTP.
        /// Range is [-9.0 .. 0.0].
        #[clap(long, default_value = "-2.0", allow_hyphen_values = true, validator=ebu_true_peak_validator)]
        true_peak: f64,

        /// Offset Gain.
        /// The gain is applied before the true-peak limiter in the first pass only.
        /// The offset for the second pass will be automatically determined based on the first pass statistics.
        /// Range is [-99.0 .. +99.0].
        #[clap(long, default_value = "0.0", allow_hyphen_values = true, validator=ebu_offset_validator)]
        offset: f64,

        /// Custom arguments for ffmpeg to override default values, e.g. "-c:a ac3 -b:a 640k -ar 48000 -dialnorm -31"
        #[clap(
            last = true,
            value_name = "ffmpeg_arguments",
            multiple_values = true,
            allow_hyphen_values = true
        )]
        ffmpeg_args: Vec<String>,
    },
    /// RMS-based normalization brings the input file to the specified RMS level.
    Rms {
        /// Normalization target level in dB/LUFS.
        /// The range is [-99.0 .. 0.0].
        #[clap(long, default_value = "-23.0", allow_hyphen_values = true, validator=rms_target_level_validator)]
        target_level: f64,

        /// Custom arguments for ffmpeg to override default values, e.g. "-c:a ac3 -b:a 640k -ar 48000 -dialnorm -31"
        #[clap(
            last = true,
            value_name = "ffmpeg_arguments",
            multiple_values = true,
            allow_hyphen_values = true
        )]
        ffmpeg_args: Vec<String>,
    },
    /// Peak normalization brings the signal to the specified peak level.
    Peak {
        /// Normalization target level in dB/LUFS.
        /// The range is [-99.0 .. 0.0].
        #[clap(long, default_value = "-23.0", allow_hyphen_values = true, validator=peak_target_level_validator)]
        target_level: f64,

        /// Custom arguments for ffmpeg to override default values, e.g. "-c:a ac3 -b:a 640k -ar 48000 -dialnorm -31"
        #[clap(
            last = true,
            value_name = "ffmpeg_arguments",
            multiple_values = true,
            allow_hyphen_values = true
        )]
        ffmpeg_args: Vec<String>,
    },
}

fn ebu_target_level_validator(s: &str) -> Result<(), String> {
    if let Ok(v) = s.parse::<f64>() {
        if (-70.0..=-5.0).contains(&v) {
            return Ok(());
        }
    }

    Err("EBU R12 target level range is [-70.0 .. -5.0].".to_string())
}

fn ebu_loudness_range_target_validator(s: &str) -> Result<(), String> {
    if let Ok(v) = s.parse::<f64>() {
        if (1.0..=20.0).contains(&v) {
            return Ok(());
        }
    }

    Err("EBU R12 loudness range target range is [+1.0 .. +20.0].".to_string())
}

fn ebu_true_peak_validator(s: &str) -> Result<(), String> {
    if let Ok(v) = s.parse::<f64>() {
        if (-9.0..=0.0).contains(&v) {
            return Ok(());
        }
    }

    Err("EBU R12 true peak range is [-9.0 .. 0.0].".to_string())
}

fn ebu_offset_validator(s: &str) -> Result<(), String> {
    if let Ok(v) = s.parse::<f64>() {
        if (-99.0..=99.0).contains(&v) {
            return Ok(());
        }
    }

    Err("EBU R12 offset range is [-99.0 .. +99.0].".to_string())
}

fn rms_target_level_validator(s: &str) -> Result<(), String> {
    if let Ok(v) = s.parse::<f64>() {
        if (-99.0..=0.0).contains(&v) {
            return Ok(());
        }
    }

    Err("RMS target level range is [-99.0 .. 0.0].".to_string())
}

fn peak_target_level_validator(s: &str) -> Result<(), String> {
    if let Ok(v) = s.parse::<f64>() {
        if (-99.0..=0.0).contains(&v) {
            return Ok(());
        }
    }

    Err("Peak target level range is [-99.0 .. 0.0].".to_string())
}
