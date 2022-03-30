mod cli;
mod ebu_r128;
mod ffmpeg;
mod ffprobe;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use ebu_r128::{normalize_ebu_r128, EbuR128NormalizationArgs};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Ebu {
            target_level,
            loudness_range_target,
            true_peak,
            offset,
            ffmpeg_args,
        } => {
            let args = EbuR128NormalizationArgs {
                verbose: cli.verbose,
                input_file: &cli.input_file,
                output_file: &cli.output_file,
                target_level,
                loudness_range_target,
                true_peak,
                offset,
                ffmpeg_args: &ffmpeg_args,
            };
            normalize_ebu_r128(args)
        }
    }
}
