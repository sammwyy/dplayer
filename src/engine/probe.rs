use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct MediaInfo {
    pub path: PathBuf,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// Duration in seconds, or `0.0` if unknown.
    pub duration: f64,
    pub has_video: bool,
    pub has_audio: bool,
    /// True if the file contains an attached picture stream (embedded cover art).
    pub attached_picture: bool,
    /// Display dimensions after applying stream rotation metadata.
    pub width: u32,
    pub height: u32,
    pub video_codec: String,
    pub audio_codec: String,
}

impl MediaInfo {
    /// Scaled output dimensions matching the `scale='min(1600,iw)':-2` video filter.
    pub fn output_size(&self) -> (u32, u32) {
        let w = self.width.min(1600);
        let h = (self.height as u64 * w as u64 / self.width.max(1) as u64) as u32;
        (w, h & !1)
    }
}

#[derive(Deserialize)]
struct Probe {
    #[serde(default)]
    streams: Vec<Stream>,
    format: Option<Format>,
}

#[derive(Deserialize)]
struct Stream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    duration: Option<String>,
    #[serde(default)]
    disposition: Disposition,
    #[serde(default)]
    side_data_list: Vec<serde_json::Value>,
}

#[derive(Default, Deserialize)]
struct Disposition {
    #[serde(default)]
    attached_pic: u32,
}

#[derive(Deserialize)]
struct Format {
    duration: Option<String>,
    #[serde(default)]
    tags: HashMap<String, String>,
}

fn tag<'a>(tags: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    tags.iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
        .filter(|value| !value.trim().is_empty())
}

fn rotation(stream: &Stream) -> i32 {
    for data in &stream.side_data_list {
        // ffprobe may encode display matrix rotation as either an integer or a string.
        let value = data
            .get("rotation")
            .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()));
        if let Some(degrees) = value {
            return degrees as i32;
        }
    }
    0
}

pub fn probe(path: &Path) -> Result<MediaInfo, String> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .output()
        .map_err(|e| format!("could not run ffprobe: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ffprobe failed: {}",
            stderr.lines().next().unwrap_or("unknown error")
        ));
    }
    let probe: Probe = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("could not parse ffprobe output: {e}"))?;

    let video = probe.streams.iter().find(|s| {
        s.codec_type.as_deref() == Some("video") && s.disposition.attached_pic == 0
    });
    let picture = probe.streams.iter().find(|s| {
        s.codec_type.as_deref() == Some("video") && s.disposition.attached_pic != 0
    });
    let audio = probe
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("audio"));

    if video.is_none() && audio.is_none() {
        return Err("no playable audio or video stream".to_string());
    }

    let format_tags = probe.format.as_ref().map(|f| &f.tags);
    let file_title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .replace(['_', '-'], " ");
    let title = format_tags
        .and_then(|t| tag(t, "title"))
        .map(str::to_string)
        .unwrap_or(file_title);

    let mut width = video.and_then(|v| v.width).unwrap_or(0);
    let mut height = video.and_then(|v| v.height).unwrap_or(0);
    if let Some(v) = video {
        match rotation(v).rem_euclid(360) {
            90 | 270 => std::mem::swap(&mut width, &mut height),
            _ => {}
        }
    }

    let duration = probe
        .format
        .as_ref()
        .and_then(|f| f.duration.as_deref())
        .or_else(|| {
            video
                .and_then(|s| s.duration.as_deref())
                .or_else(|| audio.and_then(|s| s.duration.as_deref()))
        })
        .and_then(|d| d.parse::<f64>().ok())
        .unwrap_or(0.0);

    Ok(MediaInfo {
        path: path.to_path_buf(),
        title,
        artist: format_tags
            .and_then(|t| tag(t, "artist").or_else(|| tag(t, "album_artist")))
            .map(str::to_string),
        album: format_tags.and_then(|t| tag(t, "album")).map(str::to_string),
        duration,
        has_video: video.is_some(),
        has_audio: audio.is_some(),
        attached_picture: picture.is_some(),
        width,
        height,
        video_codec: video
            .and_then(|v| v.codec_name.clone())
            .unwrap_or_default(),
        audio_codec: audio
            .and_then(|a| a.codec_name.clone())
            .unwrap_or_default(),
    })
}
