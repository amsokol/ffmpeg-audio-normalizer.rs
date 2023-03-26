use anyhow::{anyhow, bail, Context, Result};
use serde::{de::Error, Deserialize, Deserializer};
use std::env::consts::OS;
use std::env::current_dir;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub struct FFprobe {}

impl FFprobe {
    pub fn info(file: &Path) -> Result<AudioStream> {
        let output = Command::new(FFprobe::ffprobe_path())
            .arg("-i")
            .arg(file)
            .arg("-loglevel")
            .arg("error")
            .arg("-print_format")
            .arg("json")
            .arg("-show_streams")
            .arg("-select_streams")
            .arg("a:0")
            .output()
            .with_context(|| "Failed to run FFprobe")?;

        if !output.status.success() {
            let stderr = io::stderr();
            let mut lock = stderr.lock();
            let _ = writeln!(lock, "{}", String::from_utf8_lossy(&output.stderr));

            if let Some(code) = output.status.code() {
                bail!("Failed to run FFprobe with exit code={}", code);
            } else {
                bail!("Failed to run FFprobe without exit code");
            }
        }

        let mut res = serde_json::from_slice::<FileInfo>(&output.stdout)
            .with_context(|| "Failed to parse FFprobe output")?;

        res.streams
            .pop()
            .ok_or_else(|| anyhow!("FFprobe does not return stream information"))
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

#[derive(Deserialize)]
struct FileInfo {
    streams: Vec<AudioStream>,
}

#[derive(Deserialize)]
pub struct AudioStream {
    pub codec_name: String,
    #[serde(default, deserialize_with = "from_duration")]
    pub duration: Option<Duration>,
    #[serde(default)]
    pub bit_rate: Option<String>,
}

fn from_duration<'a, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
where
    D: Deserializer<'a>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    s.parse::<f64>()
        .map(|d| Some(Duration::from_micros((d * 1_000_000.0).trunc() as u64)))
        .map_err(|err| D::Error::custom(err.to_string()))
}
