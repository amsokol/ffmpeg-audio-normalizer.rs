use anyhow::{bail, Context, Result};
use props_rs::{parse, Property};
use std::env::consts::OS;
use std::env::current_dir;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub struct FFprobe {}

impl FFprobe {
    pub fn info(file: &Path) -> Result<FileInfo> {
        /* Check input file exist or not */
        if !Path::new(&file).exists() {
            bail!("File not found: {}", file.display())
        }

        let output = Command::new(FFprobe::ffprobe_path())
            .arg("-i")
            .arg(file)
            .arg("-v")
            .arg("quiet")
            .arg("-of")
            .arg("default=noprint_wrappers=1")
            .arg("-show_streams")
            .arg("-select_streams")
            .arg("a:0")
            .output()
            .with_context(|| "Failed to run ffprobe")?;

        if !output.status.success() {
            bail!(
                "Failed to run ffprobe for input file, {}, error: \n{}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )
        }

        return match parse(&output.stdout) {
            Err(error) => bail!("Failed to parse ffprobe output: {error}"),
            Ok(properties) => Ok(FileInfo { properties }),
        };
    }

    fn ffprobe_path() -> PathBuf {
        let mut path = current_dir().unwrap_or_default();
        let ffprobe = match OS {
            "windows" => "ffprobe.exe",
            _ => "ffprobe",
        };

        path.push(ffprobe);

        if !Path::new(&path).exists() {
            path.clear();
            path.push(ffprobe);
        }

        path
    }
}
pub struct FileInfo {
    properties: Vec<Property>,
}

impl FileInfo {
    fn property(&self, name: &str) -> Option<&str> {
        self.properties
            .iter()
            .find(|p| name == p.key)
            .map(|p| p.value.as_str())
    }

    pub fn codec_name(&self) -> Option<&str> {
        self.property("codec_name")
    }

    pub fn duration(&self) -> Option<Duration> {
        self.property("duration").and_then(|d| {
            d.parse::<f64>()
                .map(|d| Duration::from_millis((d * 1000.0).trunc() as u64))
                .ok()
        })
    }

    pub fn bit_rate(&self) -> Option<i64> {
        self.property("bit_rate")
            .and_then(|b| b.parse::<i64>().ok())
    }
}
