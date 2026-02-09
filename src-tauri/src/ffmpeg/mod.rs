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
    build_extract_args, build_ffmpeg_command, build_two_pass_ffmpeg_commands,
    format_args_for_display_multiline, is_preview_stream_copy_safe_codec,
    supports_two_pass_codec,
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

use serde::{Deserialize, Serialize};
use crate::error::AppError;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RateControlMode {
    Quality,
    TargetSize,
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
    pub rate_control_mode: Option<RateControlMode>,
    pub target_size_mb: Option<f64>,
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
            rate_control_mode: Some(RateControlMode::Quality),
            target_size_mb: None,
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

    pub fn effective_rate_control_mode(&self) -> RateControlMode {
        self.rate_control_mode.unwrap_or(RateControlMode::Quality)
    }

    pub fn effective_target_size_mb(&self) -> Option<f64> {
        self.target_size_mb
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
        let rate_control_mode = match self.effective_rate_control_mode() {
            RateControlMode::Quality => "quality",
            RateControlMode::TargetSize => "targetSize",
        };
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
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
            rate_control_mode,
            self.target_size_mb
                .map(|v| format!("{:.4}", v))
                .as_deref()
                .unwrap_or(""),
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

pub fn compute_target_video_bitrate_kbps(options: &TranscodeOptions) -> Result<u32, AppError> {
    if !supports_two_pass_codec(options.effective_codec()) {
        return Err(AppError::from(
            "Target size mode requires libx264, libx265, or libvpx-vp9.",
        ));
    }
    let target_size_mb = options
        .effective_target_size_mb()
        .filter(|v| v.is_finite() && *v > 0.0)
        .ok_or_else(|| AppError::from("Target size must be greater than zero"))?;
    let duration_secs = options
        .duration_secs
        .filter(|v| v.is_finite() && *v > 0.0)
        .ok_or_else(|| AppError::from("Video duration is required for target size mode"))?;

    let audio_streams = if options.effective_remove_audio() {
        0
    } else {
        let count = options.audio_stream_count.unwrap_or(1);
        if count == 0 {
            0
        } else if options.effective_preserve_additional_audio_streams() {
            count
        } else {
            1
        }
    } as f64;

    let audio_bitrate_kbps = options.effective_audio_bitrate() as f64;
    let audio_bitrate_total_kbps = audio_streams * audio_bitrate_kbps;

    let total_bits = target_size_mb * 1024.0 * 1024.0 * 8.0;
    let overhead_bits = total_bits * 0.02;
    let audio_bits = audio_bitrate_total_kbps * 1000.0 * duration_secs;
    let video_bits = total_bits - overhead_bits - audio_bits;

    if !video_bits.is_finite() || video_bits <= 0.0 {
        return Err(AppError::from("Target size is too small for audio"));
    }

    let raw_video_kbps = (video_bits / duration_secs / 1000.0).floor();
    let clamped = raw_video_kbps.clamp(200.0, 100_000.0);
    Ok(clamped as u32)
}

/// Path to string for FFmpeg args or logging.
pub fn path_to_string(path: &(impl AsRef<std::path::Path> + ?Sized)) -> String {
    path.as_ref().to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        ESTIMATE_CACHE_VERSION, RateControlMode, TranscodeOptions,
        compute_target_video_bitrate_kbps,
    };

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

    #[test]
    fn compute_target_bitrate_errors_when_audio_exceeds_target() {
        let mut opts = TranscodeOptions::default();
        opts.rate_control_mode = Some(RateControlMode::TargetSize);
        opts.target_size_mb = Some(1.0);
        opts.duration_secs = Some(60.0);
        opts.audio_bitrate = Some(320);
        opts.audio_stream_count = Some(2);
        let result = compute_target_video_bitrate_kbps(&opts);
        assert!(result.is_err());
    }

    #[test]
    fn compute_target_bitrate_returns_value() {
        let mut opts = TranscodeOptions::default();
        opts.rate_control_mode = Some(RateControlMode::TargetSize);
        opts.target_size_mb = Some(50.0);
        opts.duration_secs = Some(60.0);
        opts.audio_bitrate = Some(128);
        opts.audio_stream_count = Some(1);
        let result = compute_target_video_bitrate_kbps(&opts).unwrap();
        assert!(result >= 200);
    }
}
