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
    is_preview_stream_copy_safe_codec,
};
pub use error::{FfmpegErrorPayload, parse_ffmpeg_error};

/// Progress payload for ffmpeg-progress events.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegProgressPayload {
    pub progress: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
}
pub use cache::{
    FileSignature, cleanup_preview_transcode_cache, file_signature, get_all_cached_paths,
    get_cached_estimate, get_cached_preview, get_cached_segments, set_cached_estimate,
    set_cached_preview,
};
pub use runner::{run_ffmpeg_blocking, terminate_all_ffmpeg};
pub use temp::{
    TempFileManager, cleanup_old_temp_files, cleanup_previous_preview_paths,
    cleanup_transcode_temp, set_transcode_temp, store_preview_paths_for_cleanup,
};
#[cfg(any(test, feature = "integration-test-api"))]
pub use verify::verify_video;

use serde::Deserialize;

/// Version token for estimate cache key invalidation.
pub const ESTIMATE_CACHE_VERSION: &str = "estimate-sampled-bitrate";

/// Confidence bucket for size estimate range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EstimateConfidence {
    High,
    Medium,
    Low,
}

/// Structured output size estimate with uncertainty and sampling stats.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SizeEstimate {
    pub best_size: u64,
    pub low_size: u64,
    pub high_size: u64,
    pub confidence: EstimateConfidence,
    pub method: String,
    pub sample_count: u32,
    pub sample_seconds_total: f64,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TranscodeOptions {
    pub codec: Option<String>,
    pub quality: Option<u32>,
    pub max_bitrate: Option<u32>,
    pub scale: Option<f64>,
    pub fps: Option<f64>,
    pub remove_audio: Option<bool>,
    pub preset: Option<String>,
    pub tune: Option<String>,
    pub output_format: Option<String>,
    pub preview_duration: Option<u32>,
    pub duration_secs: Option<f64>,
    /// Include all audio streams in output (transcoded to AAC/Opus). Default false.
    pub preserve_additional_audio_streams: Option<bool>,
    /// From metadata; used when preserve_additional_audio_streams. Default 1.
    pub audio_stream_count: Option<u32>,
    /// Copy input metadata (title, creation date, etc.) to output via -map_metadata 0. Default false.
    pub preserve_metadata: Option<bool>,
    /// Audio bitrate in kbps. Default 128.
    pub audio_bitrate: Option<u32>,
    /// Downmix multichannel to stereo when output supports multichannel. Default false.
    pub downmix_to_stereo: Option<bool>,
    /// Include all subtitle streams in output. Default false.
    pub preserve_subtitles: Option<bool>,
    /// From metadata; used when preserve_subtitles. Default 0.
    pub subtitle_stream_count: Option<u32>,
    /// From metadata; first audio stream codec name for passthrough decision.
    pub audio_codec_name: Option<String>,
    /// From metadata; first audio stream channel count.
    pub audio_channels: Option<u32>,
}

impl Default for TranscodeOptions {
    fn default() -> Self {
        Self {
            codec: Some("libx264".to_string()),
            quality: Some(75),
            max_bitrate: None,
            scale: Some(1.0),
            fps: Some(30.0),
            remove_audio: Some(false),
            preset: Some("fast".to_string()),
            tune: None,
            output_format: Some("mp4".to_string()),
            preview_duration: Some(3),
            duration_secs: None,
            preserve_additional_audio_streams: None,
            audio_stream_count: None,
            preserve_metadata: None,
            audio_bitrate: None,
            downmix_to_stereo: None,
            preserve_subtitles: None,
            subtitle_stream_count: None,
            audio_codec_name: None,
            audio_channels: None,
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

    pub fn effective_fps(&self) -> f64 {
        let fps = self.fps.unwrap_or(30.0);
        (fps * 100.0).round() / 100.0
    }

    pub fn effective_remove_audio(&self) -> bool {
        self.remove_audio.unwrap_or(false)
    }

    pub fn effective_preset(&self) -> &str {
        self.preset.as_deref().unwrap_or("fast")
    }

    pub fn effective_tune(&self) -> Option<&str> {
        self.tune
            .as_deref()
            .filter(|t| !t.is_empty() && *t != "none")
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

    pub fn effective_preserve_additional_audio_streams(&self) -> bool {
        self.preserve_additional_audio_streams.unwrap_or(false)
    }

    pub fn effective_audio_stream_count(&self) -> u32 {
        self.audio_stream_count.unwrap_or(1).max(1)
    }

    pub fn effective_preserve_metadata(&self) -> bool {
        self.preserve_metadata.unwrap_or(false)
    }

    pub fn effective_audio_bitrate(&self) -> u32 {
        self.audio_bitrate.unwrap_or(128).clamp(64, 320)
    }

    pub fn effective_downmix_to_stereo(&self) -> bool {
        self.downmix_to_stereo.unwrap_or(false)
    }

    pub fn effective_preserve_subtitles(&self) -> bool {
        self.preserve_subtitles.unwrap_or(false)
    }

    pub fn effective_subtitle_stream_count(&self) -> u32 {
        self.subtitle_stream_count.unwrap_or(0)
    }

    /// Cache key for full transcode (excludes duration_secs).
    pub fn options_cache_key(&self) -> String {
        format!(
            "{}|{}",
            self.options_cache_key_common(),
            self.effective_output_format(),
        )
    }

    /// Cache key for preview (excludes output_format).
    pub fn options_cache_key_for_preview(&self) -> String {
        self.options_cache_key_common()
    }

    /// Cache key for estimate (includes output_format and estimate version).
    pub fn options_cache_key_for_estimate(&self) -> String {
        format!(
            "{}|{}|{}",
            ESTIMATE_CACHE_VERSION,
            self.options_cache_key_common(),
            self.effective_output_format()
        )
    }

    fn options_cache_key_common(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.effective_codec(),
            self.effective_quality(),
            self.max_bitrate
                .map(|b| b.to_string())
                .as_deref()
                .unwrap_or(""),
            self.effective_scale(),
            self.effective_fps(),
            self.effective_remove_audio(),
            self.effective_preset(),
            self.tune.as_deref().unwrap_or(""),
            self.effective_preserve_additional_audio_streams(),
            self.effective_audio_stream_count(),
            self.effective_preserve_metadata(),
            self.effective_audio_bitrate(),
            self.effective_downmix_to_stereo(),
            self.effective_preserve_subtitles(),
            self.effective_subtitle_stream_count(),
            self.audio_codec_name.as_deref().unwrap_or(""),
        )
    }
}

/// Path to string for FFmpeg args or logging.
pub fn path_to_string(path: &(impl AsRef<std::path::Path> + ?Sized)) -> String {
    path.as_ref().to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::{ESTIMATE_CACHE_VERSION, TranscodeOptions};

    #[test]
    fn estimate_cache_key_includes_output_format() {
        let mut opts_a = TranscodeOptions::default();
        opts_a.output_format = Some("mp4".into());
        let mut opts_b = TranscodeOptions::default();
        opts_b.output_format = Some("webm".into());

        assert_ne!(
            opts_a.options_cache_key_for_estimate(),
            opts_b.options_cache_key_for_estimate()
        );
    }

    #[test]
    fn estimate_cache_key_is_versioned() {
        let opts = TranscodeOptions::default();
        let key = opts.options_cache_key_for_estimate();
        assert!(
            key.starts_with(ESTIMATE_CACHE_VERSION),
            "estimate cache key should be prefixed with version token"
        );
    }
}
