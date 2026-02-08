//! Test-only wrappers exposed for integration test targets.

use std::path::{Path, PathBuf};

use crate::CodecInfo;
use crate::error::AppError;
use crate::ffmpeg::ffprobe::get_video_metadata_impl;
use crate::ffmpeg::{SizeEstimate, TranscodeOptions};
use crate::preview::{run_preview_core, run_preview_with_estimate_core};
use crate::{codec, ffmpeg};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewResultForTest {
    pub original_path: String,
    pub compressed_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_offset_seconds: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewWithEstimateResultForTest {
    #[serde(flatten)]
    pub preview: PreviewResultForTest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimate: Option<SizeEstimate>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoMetadataForTest {
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub size: u64,
    pub audio_stream_count: u32,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppCapabilitiesForTest {
    pub protocol_version: u8,
    pub variant: String,
    pub codecs: Vec<CodecInfo>,
}

/// Runs preview generation and returns paths for integration tests.
pub async fn run_preview_for_test(
    input_path: &Path,
    options: &TranscodeOptions,
    preview_start_seconds: Option<f64>,
) -> Result<PreviewResultForTest, AppError> {
    let result =
        run_preview_core(input_path, options, preview_start_seconds, None, None, None).await?;
    Ok(PreviewResultForTest {
        original_path: result.original_path,
        compressed_path: result.compressed_path,
        start_offset_seconds: result.start_offset_seconds,
    })
}

/// Runs preview generation with a source codec override for integration tests.
pub async fn run_preview_for_test_with_meta_codec_override(
    input_path: &Path,
    options: &TranscodeOptions,
    preview_start_seconds: Option<f64>,
    source_codec_override: &str,
) -> Result<PreviewResultForTest, AppError> {
    let mut meta = get_video_metadata_impl(input_path)?;
    meta.codec_name = Some(source_codec_override.to_string());
    let result = run_preview_core(
        input_path,
        options,
        preview_start_seconds,
        None,
        Some(meta.duration),
        Some(meta),
    )
    .await?;
    Ok(PreviewResultForTest {
        original_path: result.original_path,
        compressed_path: result.compressed_path,
        start_offset_seconds: result.start_offset_seconds,
    })
}

/// Runs preview generation with source codec + first audio codec overrides for integration tests.
pub async fn run_preview_for_test_with_meta_codec_and_audio_override(
    input_path: &Path,
    options: &TranscodeOptions,
    preview_start_seconds: Option<f64>,
    source_codec_override: &str,
    source_audio_codec_override: &str,
    source_audio_stream_count_override: u32,
) -> Result<PreviewResultForTest, AppError> {
    let mut meta = get_video_metadata_impl(input_path)?;
    meta.codec_name = Some(source_codec_override.to_string());
    meta.audio_codec_name = Some(source_audio_codec_override.to_string());
    meta.audio_stream_count = source_audio_stream_count_override;
    let result = run_preview_core(
        input_path,
        options,
        preview_start_seconds,
        None,
        Some(meta.duration),
        Some(meta),
    )
    .await?;
    Ok(PreviewResultForTest {
        original_path: result.original_path,
        compressed_path: result.compressed_path,
        start_offset_seconds: result.start_offset_seconds,
    })
}

/// Runs preview generation with estimate and returns paths + estimate for integration tests.
pub async fn run_preview_with_estimate_for_test(
    input_path: &Path,
    options: &TranscodeOptions,
    preview_start_seconds: Option<f64>,
) -> Result<PreviewWithEstimateResultForTest, AppError> {
    let result = run_preview_with_estimate_core(input_path, options, preview_start_seconds).await?;
    Ok(PreviewWithEstimateResultForTest {
        preview: PreviewResultForTest {
            original_path: result.preview.original_path,
            compressed_path: result.preview.compressed_path,
            start_offset_seconds: result.preview.start_offset_seconds,
        },
        estimate: result.estimate,
    })
}

/// Resolves video metadata through the shared backend layer.
pub fn media_inspect_metadata_for_test(path: PathBuf) -> Result<VideoMetadataForTest, String> {
    let meta = get_video_metadata_impl(&path).map_err(|e| e.to_string())?;
    Ok(VideoMetadataForTest {
        duration: meta.duration,
        width: meta.width,
        height: meta.height,
        size: meta.size,
        audio_stream_count: meta.audio_stream_count,
    })
}

/// Resolves build variant through codec discovery.
pub fn app_capabilities_for_test() -> Result<AppCapabilitiesForTest, String> {
    let available = ffmpeg::discovery::get_available_codecs().map_err(|e| e.to_string())?;
    let variant = codec::get_build_variant(available).map_err(|e| e.to_string())?;
    Ok(AppCapabilitiesForTest {
        protocol_version: 2,
        variant: variant.variant.to_string(),
        codecs: variant.codecs,
    })
}
