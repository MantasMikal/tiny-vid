mod builder;
mod cache;
pub mod discovery;
mod error;
pub mod ffprobe;
mod progress;
mod runner;
mod temp;
mod verify;

pub use builder::{
    build_extract_args, build_ffmpeg_command, format_args_for_display_multiline,
};
pub use error::{parse_ffmpeg_error, FfmpegErrorPayload};
pub use runner::{run_ffmpeg_blocking, terminate_all_ffmpeg};
pub use cache::{
    cleanup_preview_transcode_cache, get_cached_extract, get_cached_preview_transcode,
    set_cached_extract, set_cached_preview_transcode,
};
pub use temp::{
    cleanup_previous_preview_paths, cleanup_transcode_temp, set_transcode_temp,
    store_preview_paths_for_cleanup, TempFileManager,
};
#[cfg(test)]
pub use verify::verify_video;

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TranscodeOptions {
    pub codec: Option<String>,
    pub quality: Option<u32>,
    pub max_bitrate: Option<u32>,
    pub scale: Option<f64>,
    pub fps: Option<u32>,
    pub remove_audio: Option<bool>,
    pub preset: Option<String>,
    pub tune: Option<String>,
    pub output_format: Option<String>,
    pub preview_duration: Option<u32>,
    pub duration_secs: Option<f64>,
}

impl Default for TranscodeOptions {
    fn default() -> Self {
        Self {
            codec: Some("libx264".to_string()),
            quality: Some(75),
            max_bitrate: None,
            scale: Some(1.0),
            fps: Some(30),
            remove_audio: Some(false),
            preset: Some("fast".to_string()),
            tune: None,
            output_format: Some("mp4".to_string()),
            preview_duration: Some(3),
            duration_secs: None,
        }
    }
}

impl TranscodeOptions {
    pub fn effective_codec(&self) -> &str {
        self.codec.as_deref().unwrap_or("libx264")
    }

    pub fn effective_quality(&self) -> u32 {
        self.quality.unwrap_or(75)
    }

    pub fn effective_scale(&self) -> f64 {
        self.scale.unwrap_or(1.0)
    }

    pub fn effective_fps(&self) -> u32 {
        self.fps.unwrap_or(30)
    }

    pub fn effective_remove_audio(&self) -> bool {
        self.remove_audio.unwrap_or(false)
    }

    pub fn effective_preset(&self) -> &str {
        self.preset.as_deref().unwrap_or("fast")
    }

    pub fn effective_tune(&self) -> Option<&str> {
        self.tune.as_deref().filter(|t| !t.is_empty() && *t != "none")
    }

    pub fn effective_output_format(&self) -> String {
        self.output_format
            .as_deref()
            .unwrap_or("mp4")
            .to_lowercase()
    }

    pub fn effective_preview_duration(&self) -> u32 {
        self.preview_duration.unwrap_or(3)
    }

    /// Deterministic cache key from options (excludes duration_secs, for full transcode only).
    pub fn options_cache_key(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.effective_codec(),
            self.effective_quality(),
            self.max_bitrate.map(|b| b.to_string()).as_deref().unwrap_or(""),
            self.effective_scale(),
            self.effective_fps(),
            self.effective_remove_audio(),
            self.effective_preset(),
            self.tune.as_deref().unwrap_or(""),
            self.output_format.as_deref().unwrap_or(""),
        )
    }
}

/// Converts a path to a String. Use for PathBuf/Path when passing to FFmpeg or logging.
pub fn path_to_string(path: &impl AsRef<std::path::Path>) -> String {
    path.as_ref().to_string_lossy().to_string()
}
