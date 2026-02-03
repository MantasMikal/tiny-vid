//! FFprobe-based video metadata extraction. Used as a fast alternative to
//! browser video element parsing for large files.

use crate::error::AppError;
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

use super::discovery::get_ffprobe_path;

#[derive(Debug, Deserialize)]
struct FfprobeFormat {
    duration: Option<String>,
    size: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    codec_type: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    format: Option<FfprobeFormat>,
    streams: Option<Vec<FfprobeStream>>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoMetadata {
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub size: u64,
}

/// Parse ffprobe JSON output into VideoMetadata.
pub fn parse_ffprobe_json(json: &str) -> Result<VideoMetadata, AppError> {
    let output: FfprobeOutput = serde_json::from_str(json).map_err(|e| {
        AppError::from(format!("Failed to parse ffprobe JSON: {}", e))
    })?;

    let format = output.format.as_ref();
    let duration = format
        .and_then(|f| f.duration.as_ref())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let size = format
        .and_then(|f| f.size.as_ref())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let video_stream = output
        .streams
        .as_ref()
        .and_then(|streams| streams.iter().find(|s| s.codec_type.as_deref() == Some("video")));
    let width = video_stream.and_then(|s| s.width).unwrap_or(0);
    let height = video_stream.and_then(|s| s.height).unwrap_or(0);

    Ok(VideoMetadata {
        duration,
        width,
        height,
        size,
    })
}

/// Run ffprobe on a video file and return metadata.
pub fn get_video_metadata_impl(path: &Path) -> Result<VideoMetadata, AppError> {
    let ffprobe = get_ffprobe_path()?;
    let path_str = path.to_string_lossy();

    log::debug!(
        target: "tiny_vid::ffmpeg::ffprobe",
        "get_video_metadata: path={}",
        path_str
    );

    let output = Command::new(&ffprobe)
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            &path_str,
        ])
        .output()
        .map_err(|e| AppError::from(format!("Failed to run ffprobe: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::from(format!(
            "ffprobe failed: {}",
            stderr.trim()
        )));
    }

    let json = String::from_utf8(output.stdout)
        .map_err(|_| AppError::from("ffprobe output was not valid UTF-8".to_string()))?;

    parse_ffprobe_json(&json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ffprobe_json_extracts_metadata() {
        let json = r#"{
            "format": {
                "duration": "30.5",
                "size": "12345678"
            },
            "streams": [
                {
                    "codec_type": "video",
                    "width": 1920,
                    "height": 1080
                }
            ]
        }"#;
        let meta = parse_ffprobe_json(json).unwrap();
        assert_eq!(meta.duration, 30.5);
        assert_eq!(meta.width, 1920);
        assert_eq!(meta.height, 1080);
        assert_eq!(meta.size, 12_345_678);
    }

    #[test]
    fn parse_ffprobe_json_handles_missing_video_stream() {
        let json = r#"{
            "format": { "duration": "10.0", "size": "1000" },
            "streams": [{"codec_type": "audio"}]
        }"#;
        let meta = parse_ffprobe_json(json).unwrap();
        assert_eq!(meta.duration, 10.0);
        assert_eq!(meta.width, 0);
        assert_eq!(meta.height, 0);
    }

    #[test]
    fn parse_ffprobe_json_handles_empty_output() {
        let json = r#"{"format": {}, "streams": []}"#;
        let meta = parse_ffprobe_json(json).unwrap();
        assert_eq!(meta.duration, 0.0);
        assert_eq!(meta.size, 0);
        assert_eq!(meta.width, 0);
        assert_eq!(meta.height, 0);
    }
}
