use crate::io::to_stdout;
use crate::tool::ffmpeg::FFmpeg;
use crate::tool::ffprobe::FFprobe;
use anyhow::{Context, Result};
use std::path::Path;

pub struct NormalizationArgs<'a> {
    pub verbose: bool,
    pub input_file: &'a Path,
    pub output_file: &'a Path,
    pub overwrite: bool,
    pub target_level: i8,
    pub ffmpeg_args: &'a [String],
}

pub fn normalize(args: NormalizationArgs) -> Result<()> {
    // get input file information
    let input_file_info =
        FFprobe::info(args.input_file).with_context(|| "Failed to get input file information")?;

    let mut ffmpeg = FFmpeg::new(args.input_file);

    ffmpeg
        .cmd()
        .arg("-dialnorm")
        .arg(args.target_level.to_string());

    ffmpeg.add_common_args(&input_file_info, args.ffmpeg_args);

    if args.overwrite {
        ffmpeg.cmd().arg("-y");
    }
    ffmpeg.cmd().arg(args.output_file);

    let reader = ffmpeg
        .exec(
            "[1/1] Dialogue Normalizing audio file:",
            args.verbose,
            input_file_info.duration,
        )
        .with_context(|| "Failed to normalizing audio file")?;

    to_stdout(reader);

    Ok(())
}
