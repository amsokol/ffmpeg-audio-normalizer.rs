use clap::builder::RangedI64ValueParser;
use clap::builder::TypedValueParser;
use clap::{
    crate_authors, crate_description, crate_name, crate_version, AppSettings, Error, ErrorKind,
    Parser,
};
use core::ops::RangeBounds;
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
            value_parser=RangedF64ValueParser::<f64>::new().range(-70.0..=-5.0)
        )]
        target_level: f64,

        /// Loudness Range Target in LUFS.
        /// Range is [+1.0 .. +50.0].
        #[clap(
            long,
            default_value = "7.0",
            allow_hyphen_values = true,
            value_parser=RangedF64ValueParser::<f64>::new().range(1.0..=50.0)
        )]
        loudness_range_target: f64,

        /// Maximum True Peak in dBTP.
        /// Range is [-9.0 .. 0.0].
        #[clap(
            long,
            default_value = "-2.0",
            allow_hyphen_values = true,
            value_parser=RangedF64ValueParser::<f64>::new().range(-9.0..=0.0)
        )]
        true_peak: f64,

        /// Offset Gain.
        /// The gain is applied before the true-peak limiter in the first pass only.
        /// The offset for the second pass will be automatically determined based on the first pass statistics.
        /// Range is [-99.0 .. +99.0].
        #[clap(
            long,
            default_value = "0.0",
            allow_hyphen_values = true,
            value_parser=RangedF64ValueParser::<f64>::new().range(-99.0..=99.0)
        )]
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
        #[clap(
            long,
            default_value = "-23.0",
            allow_hyphen_values = true,
            value_parser=RangedF64ValueParser::<f64>::new().range(-99.0..=0.0)
        )]
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
        #[clap(
            long,
            default_value = "-23.0",
            allow_hyphen_values = true,
            value_parser=RangedF64ValueParser::<f64>::new().range(-99.0..=0.0)
        )]
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
    /// Dialogue normalization indicates how far the average dialogue level of the program is below digital 100%
    /// full scale (0 dBFS).
    Dialogue {
        /// Dialogue normalization target level determines a level shift during audio reproduction
        /// that sets the average volume of the dialogue to a preset level.
        /// The goal is to match volume level between program sources.
        /// A value of -31dB will result in no volume level change, relative to the source volume,
        /// during audio reproduction. Valid values are whole numbers in the range -31 to -1.
        #[clap(
            long,
            default_value = "-31",
            allow_hyphen_values = true,
            value_parser=RangedI64ValueParser::<i8>::new().range(-31..=-1)
        )]
        target_level: i8,

        /// Custom arguments for ffmpeg to override default values, e.g. "-c:a ac3 -b:a 640k -ar 48000"
        #[clap(
            last = true,
            value_name = "ffmpeg_arguments",
            multiple_values = true,
            allow_hyphen_values = true
        )]
        ffmpeg_args: Vec<String>,
    },
}

#[derive(Copy, Clone, Debug)]
pub struct RangedF64ValueParser<T: TryFrom<f64> = f64> {
    bounds: (std::ops::Bound<f64>, std::ops::Bound<f64>),
    target: std::marker::PhantomData<T>,
}

impl<T: TryFrom<f64>> RangedF64ValueParser<T> {
    /// Select full range of `f64`
    pub fn new() -> Self {
        Self::from(..)
    }

    /// Narrow the supported range
    pub fn range<B: RangeBounds<f64>>(mut self, range: B) -> Self {
        // Consideration: when the user does `value_parser!(f32).range()`
        // - Avoid programming mistakes by accidentally expanding the range
        // - Make it convenient to limit the range like with `..10`
        let start = match range.start_bound() {
            l @ std::ops::Bound::Included(_) => l.cloned(),
            l @ std::ops::Bound::Excluded(_) => l.cloned(),
            std::ops::Bound::Unbounded => self.bounds.start_bound().cloned(),
        };
        let end = match range.end_bound() {
            l @ std::ops::Bound::Included(_) => l.cloned(),
            l @ std::ops::Bound::Excluded(_) => l.cloned(),
            std::ops::Bound::Unbounded => self.bounds.end_bound().cloned(),
        };
        self.bounds = (start, end);
        self
    }

    fn format_bounds(&self) -> String {
        let mut result = match self.bounds.0 {
            std::ops::Bound::Included(i) => i.to_string(),
            std::ops::Bound::Excluded(i) => i.to_string(),
            std::ops::Bound::Unbounded => f64::MIN.to_string(),
        };
        result.push_str("..");
        match self.bounds.1 {
            std::ops::Bound::Included(i) => {
                result.push('=');
                result.push_str(&i.to_string());
            }
            std::ops::Bound::Excluded(i) => {
                result.push_str(&i.to_string());
            }
            std::ops::Bound::Unbounded => {
                result.push_str(&f64::MAX.to_string());
            }
        }
        result
    }
}

impl<T: TryFrom<f64> + Clone + Send + Sync + 'static> TypedValueParser
    for RangedF64ValueParser<T>
where
    <T as TryFrom<f64>>::Error: Send + Sync + 'static + std::error::Error + ToString,
{
    type Value = f64;

    fn parse_ref(
        &self,
        _: &clap::Command,
        arg: Option<&clap::Arg>,
        raw_value: &std::ffi::OsStr,
    ) -> Result<Self::Value, Error> {
        let value = raw_value.to_str().ok_or_else(|| {
            Error::raw(
                ErrorKind::InvalidUtf8,
                format!(
                    "Invalid value \"{}\" for '{}'",
                    raw_value.to_string_lossy().into_owned(),
                    arg.unwrap_or(&clap::Arg::new("<unknown argument>"))
                ),
            )
        })?;

        let value = value.parse::<f64>().map_err(|err| {
            Error::raw(
                ErrorKind::InvalidValue,
                format!(
                    "Invalid value \"{}\" for '{}': {}",
                    raw_value.to_string_lossy().into_owned(),
                    arg.unwrap_or(&clap::Arg::new("<unknown argument>")),
                    err
                ),
            )
        })?;

        if !self.bounds.contains(&value) {
            return Err(Error::raw(
                ErrorKind::InvalidValue,
                format!(
                    "Invalid value \"{}\" for '{}': {} is not in {}",
                    raw_value.to_string_lossy().into_owned(),
                    arg.unwrap_or(&clap::Arg::new("<unknown argument>")),
                    value,
                    self.format_bounds()
                ),
            ));
        }

        Ok(value)
    }
}

impl<T: TryFrom<f64>, B: RangeBounds<f64>> From<B> for RangedF64ValueParser<T> {
    fn from(range: B) -> Self {
        Self {
            bounds: (range.start_bound().cloned(), range.end_bound().cloned()),
            target: Default::default(),
        }
    }
}

impl<T: TryFrom<f64>> Default for RangedF64ValueParser<T> {
    fn default() -> Self {
        Self::new()
    }
}
