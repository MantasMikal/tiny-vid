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
    #[serde(default)]
    start_time: Option<String>,
    size: Option<String>,
    #[serde(default)]
    bit_rate: Option<String>,
    #[serde(default)]
    format_name: Option<String>,
    #[serde(default)]
    format_long_name: Option<String>,
    #[serde(default)]
    nb_streams: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    codec_type: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    r_frame_rate: Option<String>,
    #[serde(default)]
    codec_name: Option<String>,
    #[serde(default)]
    codec_long_name: Option<String>,
    #[serde(default)]
    bit_rate: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    format: Option<FfprobeFormat>,
    streams: Option<Vec<FfprobeStream>>,
}

fn parse_frame_rate(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() != 2 {
        return None;
    }
    let num: f64 = parts[0].trim().parse().ok()?;
    let den: f64 = parts[1].trim().parse().ok()?;
    if den == 0.0 {
        return None;
    }
    Some(num / den)
}

fn parse_bit_rate(s: &str) -> Option<u64> {
    s.trim().parse().ok()
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoMetadata {
    pub duration: f64,
    /// Format start_time (seconds). Non-zero for stream-copied segments; re-encoded typically 0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<f64>,
    pub width: u32,
    pub height: u32,
    pub size: u64,
    pub fps: f64,
    pub codec_name: Option<String>,
    pub codec_long_name: Option<String>,
    pub video_bit_rate: Option<u64>,
    pub format_bit_rate: Option<u64>,
    pub format_name: Option<String>,
    pub format_long_name: Option<String>,
    pub nb_streams: Option<u32>,
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
    let start_time = format
        .and_then(|f| f.start_time.as_ref())
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|&t| t > 0.0);
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
    let fps = video_stream
        .and_then(|s| s.r_frame_rate.as_deref())
        .and_then(parse_frame_rate)
        .unwrap_or(0.0);

    let codec_name = video_stream.and_then(|s| s.codec_name.clone());
    let codec_long_name = video_stream.and_then(|s| s.codec_long_name.clone());
    let video_bit_rate = video_stream
        .and_then(|s| s.bit_rate.as_deref())
        .and_then(parse_bit_rate);
    let format_bit_rate = format
        .and_then(|f| f.bit_rate.as_deref())
        .and_then(parse_bit_rate);
    let format_name = format.and_then(|f| f.format_name.clone());
    let format_long_name = format.and_then(|f| f.format_long_name.clone());
    let nb_streams = format.and_then(|f| f.nb_streams);

    Ok(VideoMetadata {
        duration,
        start_time,
        width,
        height,
        size,
        fps,
        codec_name,
        codec_long_name,
        video_bit_rate,
        format_bit_rate,
        format_name,
        format_long_name,
        nb_streams,
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
                    "height": 1080,
                    "r_frame_rate": "30/1"
                }
            ]
        }"#;
        let meta = parse_ffprobe_json(json).unwrap();
        assert_eq!(meta.duration, 30.5);
        assert_eq!(meta.width, 1920);
        assert_eq!(meta.height, 1080);
        assert_eq!(meta.size, 12_345_678);
        assert!((meta.fps - 30.0).abs() < 0.01);
    }

    #[test]
    fn parse_ffprobe_json_extracts_extended_metadata() {
        let json = r#"{
            "format": {
                "duration": "60.0",
                "size": "50000000",
                "bit_rate": "6666666",
                "format_name": "mp4",
                "format_long_name": "QuickTime / MOV",
                "nb_streams": 2
            },
            "streams": [
                {
                    "codec_type": "video",
                    "width": 1280,
                    "height": 720,
                    "r_frame_rate": "24/1",
                    "codec_name": "h264",
                    "codec_long_name": "H.264 / AVC",
                    "bit_rate": "5000000"
                }
            ]
        }"#;
        let meta = parse_ffprobe_json(json).unwrap();
        assert_eq!(meta.codec_name.as_deref(), Some("h264"));
        assert_eq!(meta.codec_long_name.as_deref(), Some("H.264 / AVC"));
        assert_eq!(meta.video_bit_rate, Some(5_000_000));
        assert_eq!(meta.format_bit_rate, Some(6_666_666));
        assert_eq!(meta.format_name.as_deref(), Some("mp4"));
        assert_eq!(meta.format_long_name.as_deref(), Some("QuickTime / MOV"));
        assert_eq!(meta.nb_streams, Some(2));
    }

    #[test]
    fn parse_frame_rate_24000_1001() {
        let fps = parse_frame_rate("24000/1001").unwrap();
        assert!((fps - 23.976).abs() < 0.001);
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
        assert_eq!(meta.fps, 0.0);
    }

    #[test]
    fn parse_ffprobe_json_handles_empty_output() {
        let json = r#"{"format": {}, "streams": []}"#;
        let meta = parse_ffprobe_json(json).unwrap();
        assert_eq!(meta.duration, 0.0);
        assert_eq!(meta.size, 0);
        assert_eq!(meta.width, 0);
        assert_eq!(meta.height, 0);
        assert_eq!(meta.fps, 0.0);
    }

    #[test]
    fn parse_ffprobe_json_extracts_start_time() {
        let json = r#"{
            "format": {
                "duration": "3.085",
                "start_time": "0.083000",
                "size": "4402439"
            },
            "streams": [{"codec_type": "video", "width": 1920, "height": 960, "r_frame_rate": "24000/1001"}]
        }"#;
        let meta = parse_ffprobe_json(json).unwrap();
        assert_eq!(meta.start_time, Some(0.083));
    }
}
