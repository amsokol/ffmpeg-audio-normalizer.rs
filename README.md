# ffmpeg-audio-normalizer.rs

A utility for normalizing audio using ffmpeg.

Inspired by [ffmpeg-normalize](https://github.com/slhck/ffmpeg-normalize) Python tool.
All credits go to [@slhck](https://github.com/slhck).

This program normalizes media files to a certain loudness level using the EBU R128 loudness normalization procedure. It can also perform RMS-based normalization (where the mean is lifted or attenuated), or peak normalization to a certain target level.

**A very quick how-to:**

1. Install `ffmpeg` and `ffprobe` from <https://ffmpeg.org/>
1. Run `ffmpeg-audio-normalizer -i /path/to/your/audio.ac3 -o /path/to/your/audio.ebu-r128.ac3 ebu`
1. Done! 🎧

Read on for more info.

- [Requirements](#requirements)
- [Installation](#installation)
- [Usage](#usage)
- [Examples](#examples)
- [Description](#description)
- [Detailed Options](#detailed-options)
  - [General](#general)
  - [File Input/Output](#file-inputoutput)
  - [EBU R128 normalization (`ebu` subcommand)](#ebu-r128-normalization-ebu-subcommand)
  - [RMS-based normalization (`rms` subcommand)](#rms-based-normalization-rms-subcommand)
  - [Peak normalization (`peak` subcommand)](#peak-normalization-peak-subcommand)
  - [FFmpeg parameters](#ffmpeg-parameters)

## Requirements

- ffmpeg and ffprobe v4.2 or higher from <https://ffmpeg.org/> – static builds using the latest Git master are recommended
- `ffmpeg` and `ffprobe` must be in your \$PATH or in the current folder

## Installation

Build or download from [Releases](https://github.com/amsokol/ffmpeg-audio-normalizer.rs/releases) `ffmpeg-audio-normalizer` executable.

## Usage

    USAGE:
        ffmpeg-audio-normalizer [OPTIONS] --input-file <INPUT_FILE> --output-file <OUTPUT_FILE> <SUBCOMMAND>

    OPTIONS:
            --verbose                      Verbose output
        -i, --input-file <INPUT_FILE>      Input audio file
        -o, --output-file <OUTPUT_FILE>    Output audio file after normalization
            --overwrite                    Force overwrite existing output file
        -h, --help                         Print help information
        -V, --version                      Print version information

    SUBCOMMANDS:
        ebu     EBU normalization performs two passes and normalizes according to EBU R128
        rms     RMS-based normalization brings the input file to the specified RMS level
        peak    Peak normalization brings the signal to the specified peak level
        help    Print this message or the help of the given subcommand(s)

For more information, run `ffmpeg-audio-normalizer -h`, or read on.

## Examples

    ffmpeg-audio-normalizer -i /path/to/your/audio.ac3 -o /path/to/your/audio.ebu-r128.ac3 ebu -- -dialnorm -31

    ffmpeg-audio-normalizer --verbose -i /path/to/your/audio.dts -o /path/to/your/audio.ebu-r128.eac3 rms -- -c:a eac3 -b:a 1509k -ar 48000 -dialnorm -31

    ffmpeg-audio-normalizer -i /path/to/your/audio.dts -o /path/to/your/audio.ebu-r128.eac3 --overwrite peak --target-level 0 -- -c:a eac3 -b:a 1509k -ar 48000 -dialnorm -31

## Description

**How will the normalization be done?**

The normalization will be performed with the [`loudnorm` filter](http://ffmpeg.org/ffmpeg-filters.html#loudnorm) from FFmpeg, which was [originally written by Kyle Swanson](https://k.ylo.ph/2016/04/04/loudnorm.html). It will bring the audio to a specified target level. This ensures that multiple files normalized with this filter will have the same perceived loudness.

## Detailed Options

### General

- `--verbose`: Print verbose output
- `-h, --help`: Print help information
- `-V, --version`: Print version information

### File Input/Output

- `-i, --input-file <INPUT_FILE>`: Input audio file
- `-o, --output-file <OUTPUT_FILE>`: Output audio file after normalization

### EBU R128 normalization (`ebu` subcommand)

Performs two passes and normalizes according to EBU R128.

Run for details:

    ffmpeg-audio-normalizer help ebu

Options:

- `--target-level`: Normalization target level in dB/LUFS. It corresponds to Integrated Loudness Target in LUFS. The range is [-70.0 .. -5.0] [default: -23.0]

- `--loudness-range-target`: Loudness Range Target in LUFS. Range is [+1.0 .. +20.0] [default: 7.0]

- `--true-peak`: Maximum True Peak in dBTP. Range is [-9.0 .. 0.0] [default: -2.0]

- `--offset`: Offset Gain. The gain is applied before the true-peak limiter in the first pass only. The offset for the second pass will be automatically determined based on the first pass statistics. Range is [-99.0 .. +99.0] [default: 0.0]

### RMS-based normalization (`rms` subcommand)

RMS-based normalization brings the input file to the specified RMS level.

Run for details:

    ffmpeg-audio-normalizer help rms

Options:

- `--target-level`: Normalization target level in dB/LUFS. The range is [-99.0 .. 0.0] [default: -23.0]

### Peak normalization (`peak` subcommand)

Peak normalization brings the signal to the specified peak level.

Run for details:

    ffmpeg-audio-normalizer help peak

Options:

- `--target-level`: Normalization target level in dB/LUFS. The range is [-99.0 .. 0.0] [default: -23.0]

### FFmpeg parameters

- `--` A list of extra ffmpeg command line arguments after.

Example:

    ffmpeg-audio-normalizer -i ./audio.ac3 -o ./audio.ebu-r128.eac3 ebu -- -c:a eac3 -b:a 1509k -ar 48000 -dialnorm -31

**NOTES**: PowerShell for Windows splits parameter with `:`. So therefore such parameters must be in quotes. Example:

    ffmpeg-audio-normalizer -i ./audio.ac3 -o ./audio.ebu-r128.eac3 ebu -- "-c:a" eac3 "-b:a" 1509k -ar 48000 -dialnorm -31
