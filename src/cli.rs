use clap::{crate_authors, crate_description, crate_name, crate_version, AppSettings, Parser};
use std::path::PathBuf;

/*
    /// Normalization type (default: `ebu`).
    /// Valid values include 'ebu', 'rms', 'peak'.
    /// EBU normalization performs two passes and normalizes according to EBU R128.
    /// RMS-based normalization brings the input file to the specified RMS level.
    /// Peak normalization brings the signal to the specified peak level.
    #[clap(long, ignore_case = true, possible_values = ["ebu"], default_value = "ebu")]
    normalization_type: String,
*/

#[derive(Parser, Debug)]
#[clap(name = crate_name!())]
#[clap(author = crate_authors!("\n"))]
#[clap(version = crate_version!())]
#[clap(about = crate_description!(), long_about = None)]
#[clap(allow_negative_numbers = true)]
#[clap(global_setting(AppSettings::DeriveDisplayOrder))]
pub struct Cli {
    /// Input audio file
    #[clap(long, short, value_name = "INPUT_FILE", parse(from_os_str))]
    pub input_file: PathBuf,

    /// Output audio file after normalization
    #[clap(long, short, value_name = "OUTPUT_FILE", parse(from_os_str))]
    pub output_file: PathBuf,

    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Parser, Debug)]
pub enum Command {
    /// EBU normalization performs two passes and normalizes according to EBU R128.
    Ebu {
        /// Normalization target level in dB/LUFS (default: -23).
        /// For EBU normalization, it corresponds to Integrated Loudness Target in LUFS.
        /// The range is -70.0 - -5.0.
        /// Otherwise, the range is -99 to 0.
        #[clap(long, default_value = "-23.0")]
        target_level: f64,

        /// EBU Loudness Range Target in LUFS (default: 7.0).
        /// Range is 1.0 - 20.0.
        #[clap(long, default_value = "7.0")]
        loudness_range_target: f64,

        /// EBU Maximum True Peak in dBTP (default: -2.0).
        /// Range is -9.0 - +0.0.
        #[clap(long, default_value = "-2.0")]
        true_peak: f64,

        /// EBU Offset Gain (default: 0.0).
        /// The gain is applied before the true-peak limiter in the first pass only.
        /// The offset for the second pass will be automatically determined based on the first pass statistics.
        /// Range is -99.0 - +99.0.
        #[clap(long, default_value = "0.0")]
        offset: f64,

        /// Custom arguments for ffmpeg, e.g. "-c:a ac3 -b:a 640k -ar 48000 -dialnorm -31"
        #[clap(
            last = true,
            value_name = "ffmpeg_arguments",
            multiple_values = true,
            allow_hyphen_values = true
        )]
        ffmpeg_args: Vec<String>,
    },
}
