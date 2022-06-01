mod algorithm;
mod cli;
mod tool;

use algorithm::dialogue;
use algorithm::ebu_r128;
use algorithm::peak;
use algorithm::rms;
use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};

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
            let args = ebu_r128::NormalizationArgs {
                verbose: cli.verbose,
                input_file: &cli.input_file,
                output_file: &cli.output_file,
                overwrite: cli.overwrite,
                target_level,
                loudness_range_target,
                true_peak,
                offset,
                ffmpeg_args: &ffmpeg_args,
            };
            ebu_r128::normalize(args)
        }
        Command::Rms {
            target_level,
            ffmpeg_args,
        } => rms::normalize(rms::NormalizationArgs {
            verbose: cli.verbose,
            input_file: &cli.input_file,
            output_file: &cli.output_file,
            overwrite: cli.overwrite,
            target_level,
            ffmpeg_args: &ffmpeg_args,
        }),
        Command::Peak {
            target_level,
            ffmpeg_args,
        } => peak::normalize(peak::NormalizationArgs {
            verbose: cli.verbose,
            input_file: &cli.input_file,
            output_file: &cli.output_file,
            overwrite: cli.overwrite,
            target_level,
            ffmpeg_args: &ffmpeg_args,
        }),
        Command::Dialogue {
            target_level,
            ffmpeg_args,
        } => dialogue::normalize(dialogue::NormalizationArgs {
            verbose: cli.verbose,
            input_file: &cli.input_file,
            output_file: &cli.output_file,
            overwrite: cli.overwrite,
            target_level,
            ffmpeg_args: &ffmpeg_args,
        }),
    }
}
