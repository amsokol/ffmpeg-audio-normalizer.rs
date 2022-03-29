mod cli;
mod ebu;
mod ffmpeg;
mod ffprobe;

use anyhow::{bail, Context, Result};
use clap::Parser;
use cli::{Cli, Command};
use ebu::normalize_ebu;
use ffprobe::file_info;

fn main() -> Result<()> {
    let cli = Cli::parse();

    // get input file information
    let input_file_info =
        file_info(&cli.input_file).with_context(|| "Failed to get input file information")?;

    match cli.command {
        Command::Ebu {
            target_level,
            loudness_range_target,
            true_peak,
            offset,
            ffmpeg_args,
        } => normalize_ebu(
            &cli.input_file,
            input_file_info,
            target_level,
            loudness_range_target,
            true_peak,
            offset,
            &ffmpeg_args,
        ),
    }
}
