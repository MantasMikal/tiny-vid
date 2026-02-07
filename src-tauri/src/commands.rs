//! Tauri command handlers.

use std::fs;
use std::io;
use std::path::PathBuf;

use crate::AppState;
use crate::codec::BuildVariantResult;
use crate::error::AppError;
use crate::ffmpeg::ffprobe::{VideoMetadata as FfprobeVideoMetadata, get_video_metadata_impl};
use crate::ffmpeg::{
    TempFileManager, TranscodeOptions, build_ffmpeg_command, cleanup_transcode_temp,
    format_args_for_display_multiline, path_to_string, set_transcode_temp, terminate_all_ffmpeg,
};
use crate::preview::{PreviewWithEstimateResult, run_preview_core, run_preview_with_estimate_core};
use tauri::{Emitter, Manager};

fn is_cross_device_rename_error(e: &io::Error) -> bool {
    #[cfg(unix)]
    {
        e.raw_os_error() == Some(18) // EXDEV
    }
    #[cfg(windows)]
    {
        e.raw_os_error() == Some(17) // ERROR_NOT_SAME_DEVICE
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = e;
        false
    }
}

fn resolve_preview_media_path(path: &PathBuf) -> Option<PathBuf> {
    let canonical = fs::canonicalize(path).ok()?;
    let temp_dir = fs::canonicalize(std::env::temp_dir()).ok()?;
    let file_name = canonical.file_name()?.to_str()?;
    let is_tiny_vid_temp_mp4 = file_name.starts_with("tiny-vid-") && file_name.ends_with(".mp4");
    if canonical.starts_with(&temp_dir) && is_tiny_vid_temp_mp4 {
        Some(canonical)
    } else {
        None
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VideoMetadataResult {
    duration: f64,
    width: u32,
    height: u32,
    size: u64,
    size_mb: f64,
    fps: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    codec_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    codec_long_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    video_bit_rate: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    format_bit_rate: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    format_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    format_long_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nb_streams: Option<u32>,
    audio_stream_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    subtitle_stream_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audio_codec_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audio_channels: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    encoder: Option<String>,
}

impl From<FfprobeVideoMetadata> for VideoMetadataResult {
    fn from(meta: FfprobeVideoMetadata) -> Self {
        let fps = (meta.fps * 100.0).round() / 100.0;
        Self {
            duration: meta.duration,
            width: meta.width,
            height: meta.height,
            size: meta.size,
            size_mb: meta.size as f64 / 1024.0 / 1024.0,
            fps,
            codec_name: meta.codec_name,
            codec_long_name: meta.codec_long_name,
            video_bit_rate: meta.video_bit_rate,
            format_bit_rate: meta.format_bit_rate,
            format_name: meta.format_name,
            format_long_name: meta.format_long_name,
            nb_streams: meta.nb_streams,
            audio_stream_count: meta.audio_stream_count,
            subtitle_stream_count: Some(meta.subtitle_stream_count),
            audio_codec_name: meta.audio_codec_name,
            encoder: meta.encoder,
            audio_channels: meta.audio_channels,
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn ffmpeg_transcode_to_temp(
    input_path: PathBuf,
    options: TranscodeOptions,
    app: tauri::AppHandle,
    window: tauri::Window,
) -> Result<String, AppError> {
    log::info!(
        target: "tiny_vid::commands",
        "ffmpeg_transcode_to_temp: input={}",
        input_path.display()
    );
    cleanup_transcode_temp();

    let ext = options.effective_output_format();
    let suffix = format!("transcode-output.{}", ext);

    let temp = TempFileManager;
    let output_path = temp.create(&suffix, None).map_err(AppError::from)?;
    let output_str = path_to_string(&output_path);

    set_transcode_temp(Some(output_path.clone()));

    let args = build_ffmpeg_command(
        &path_to_string(&input_path),
        &output_str,
        &options,
        None,
        None,
        None,
    )?;
    let duration_secs = options.duration_secs;
    let window_label = window.label().to_string();
    let progress_callback =
        crate::preview::make_progress_emitter(app.clone(), window_label.clone(), "transcode");

    match crate::preview::run_ffmpeg_step(
        args,
        Some((&app, &window_label)),
        duration_secs,
        Some(progress_callback),
    )
    .await
    {
        Ok(()) => {
            log::info!(
                target: "tiny_vid::commands",
                "ffmpeg_transcode_to_temp: complete -> {}",
                output_str
            );
            let _ = app.emit_to(&window_label, "ffmpeg-complete", ());
            Ok(output_str)
        }
        Err(e) => {
            cleanup_transcode_temp();
            Err(e)
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn ffmpeg_preview(
    input_path: PathBuf,
    options: TranscodeOptions,
    preview_start_seconds: Option<f64>,
    include_estimate: bool,
    app: tauri::AppHandle,
    window: tauri::Window,
) -> Result<PreviewWithEstimateResult, AppError> {
    let emit = Some((app, window.label().to_string()));
    if include_estimate {
        let result =
            run_preview_with_estimate_core(&input_path, &options, preview_start_seconds, emit)
                .await?;
        Ok(result)
    } else {
        let result = run_preview_core(
            &input_path,
            &options,
            preview_start_seconds,
            emit,
            None,
            None,
            None,
        )
        .await?;
        Ok(PreviewWithEstimateResult {
            preview: result,
            estimate: None,
        })
    }
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_file_size(path: PathBuf) -> Result<u64, AppError> {
    log::debug!(
        target: "tiny_vid::commands",
        "get_file_size: path={}",
        path.display()
    );
    fs::metadata(&path).map(|m| m.len()).map_err(Into::into)
}

#[tauri::command(rename_all = "camelCase")]
pub fn preview_media_bytes(path: PathBuf) -> Result<Vec<u8>, AppError> {
    log::debug!(
        target: "tiny_vid::commands",
        "preview_media_bytes: path={}",
        path.display()
    );
    let allowed = resolve_preview_media_path(&path)
        .ok_or_else(|| AppError::from("Preview media path is not allowed"))?;
    fs::read(allowed).map_err(Into::into)
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_video_metadata(path: PathBuf) -> Result<VideoMetadataResult, AppError> {
    log::debug!(
        target: "tiny_vid::commands",
        "get_video_metadata: path={}",
        path.display()
    );
    let meta = get_video_metadata_impl(&path)?;
    Ok(meta.into())
}

#[tauri::command(rename_all = "camelCase")]
pub fn preview_ffmpeg_command(options: TranscodeOptions, input_path: Option<String>) -> String {
    let input_str = input_path.as_deref().unwrap_or("<input>");
    let output_str = "<output>";
    let args = build_ffmpeg_command(input_str, output_str, &options, None, None, None)
        .unwrap_or_else(|e| vec!["# error".into(), e.to_string()]);
    format!("ffmpeg\n{}", format_args_for_display_multiline(&args))
}

#[tauri::command]
pub fn ffmpeg_terminate() {
    log::info!(
        target: "tiny_vid::commands",
        "ffmpeg_terminate: terminating all FFmpeg processes"
    );
    terminate_all_ffmpeg();
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_pending_opened_files(state: tauri::State<'_, AppState>) -> Vec<String> {
    let mut files = state.pending_opened_files.lock();
    files.drain(..).map(|p| path_to_string(&p)).collect()
}

pub fn buffer_opened_files(app: &tauri::AppHandle, files: Vec<PathBuf>) {
    if files.is_empty() {
        return;
    }
    let asset_scope = app.asset_protocol_scope();
    for file in &files {
        let _ = asset_scope.allow_file(file);
    }
    let paths: Vec<String> = files.iter().map(path_to_string).collect();
    {
        let state = app.state::<AppState>();
        let mut pending = state.pending_opened_files.lock();
        pending.extend(files);
    }
    let _ = app.emit("open-file", paths);
}

#[tauri::command(rename_all = "camelCase")]
pub fn move_compressed_file(source: PathBuf, dest: PathBuf) -> Result<(), AppError> {
    log::info!(
        target: "tiny_vid::commands",
        "move_compressed_file: {} -> {}",
        source.display(),
        dest.display()
    );
    match fs::rename(&source, &dest) {
        Ok(()) => {
            log::debug!(target: "tiny_vid::commands", "move_compressed_file: complete");
            Ok(())
        }
        Err(e) => {
            if is_cross_device_rename_error(&e) {
                fs::copy(&source, &dest)?;
                fs::remove_file(&source)?;
                return Ok(());
            }
            Err(e.into())
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub fn cleanup_temp_file(path: PathBuf) -> Result<(), AppError> {
    log::info!(
        target: "tiny_vid::commands",
        "cleanup_temp_file: path={}",
        path.display()
    );
    let _ = fs::remove_file(&path);
    cleanup_transcode_temp();
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_build_variant() -> Result<BuildVariantResult, AppError> {
    let available = crate::ffmpeg::discovery::get_available_codecs()?;
    crate::codec::get_build_variant(available)
}
