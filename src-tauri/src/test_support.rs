//! Test-only wrappers exposed for integration test targets.

use std::path::{Path, PathBuf};

use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, mock_builder, mock_context, noop_assets};
use tauri::webview::InvokeRequest;

use crate::CodecInfo;
use crate::commands;
use crate::error::AppError;
use crate::ffmpeg::ffprobe::get_video_metadata_impl;
use crate::ffmpeg::{SizeEstimate, TranscodeOptions};
use crate::preview::{run_preview_core, run_preview_with_estimate_core};

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
pub struct BuildVariantForTest {
    pub variant: String,
    pub codecs: Vec<CodecInfo>,
}

/// Runs preview generation and returns paths for integration tests.
pub async fn run_preview_for_test(
    input_path: &Path,
    options: &TranscodeOptions,
    preview_start_seconds: Option<f64>,
) -> Result<PreviewResultForTest, AppError> {
    let result = run_preview_core(
        input_path,
        options,
        preview_start_seconds,
        None,
        None,
        None,
        None,
    )
    .await?;
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
    let result =
        run_preview_with_estimate_core(input_path, options, preview_start_seconds, None).await?;
    Ok(PreviewWithEstimateResultForTest {
        preview: PreviewResultForTest {
            original_path: result.preview.original_path,
            compressed_path: result.preview.compressed_path,
            start_offset_seconds: result.preview.start_offset_seconds,
        },
        estimate: result.estimate,
    })
}

/// Invokes get_video_metadata through the Tauri command layer.
pub fn get_video_metadata_via_command_for_test(
    path: PathBuf,
) -> Result<VideoMetadataForTest, String> {
    let app = create_test_app_for_commands();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .map_err(|e| e.to_string())?;
    let body = InvokeBody::from(serde_json::json!({
        "path": path.to_string_lossy()
    }));
    let response =
        tauri::test::get_ipc_response(&window, invoke_request("get_video_metadata", body))
            .map_err(|e| format!("{:?}", e))?;
    response.deserialize().map_err(|e| e.to_string())
}

/// Invokes get_build_variant through the Tauri command layer.
pub fn get_build_variant_via_command_for_test() -> Result<BuildVariantForTest, String> {
    let app = create_test_app_for_commands();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .map_err(|e| e.to_string())?;
    let response = tauri::test::get_ipc_response(
        &window,
        invoke_request("get_build_variant", InvokeBody::default()),
    )
    .map_err(|e| format!("{:?}", e))?;
    response.deserialize().map_err(|e| e.to_string())
}

fn create_test_app_for_commands() -> tauri::App<tauri::test::MockRuntime> {
    mock_builder()
        .manage(crate::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::get_file_size,
            commands::get_video_metadata,
            commands::get_build_variant,
            commands::ffmpeg_terminate,
            commands::move_compressed_file,
            commands::cleanup_temp_file,
            commands::get_pending_opened_files,
        ])
        .build(mock_context(noop_assets()))
        .expect("failed to build test app")
}

fn invoke_request(cmd: &str, body: InvokeBody) -> InvokeRequest {
    InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: "http://tauri.localhost".parse().expect("valid URL"),
        body,
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    }
}
