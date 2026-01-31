mod error;
mod ffmpeg;

use error::AppError;
use tauri::Emitter;
use ffmpeg::{
    build_ffmpeg_command, cleanup_previous_preview_paths, cleanup_transcode_temp, parse_ffmpeg_error,
    run_ffmpeg_blocking, set_transcode_temp, store_preview_paths_for_cleanup, terminate_all_ffmpeg,
    TempFileManager, TranscodeOptions,
};
use ffmpeg::FfmpegErrorPayload;

fn build_error_payload(e: &AppError) -> FfmpegErrorPayload {
    let (stderr, code) = match e {
        AppError::FfmpegFailed { code, stderr } => (stderr.clone(), Some(*code)),
        _ => (e.to_string(), None),
    };
    parse_ffmpeg_error(&stderr, code)
}
use std::fs;
use std::path::PathBuf;

/// Run FFmpeg in a blocking task, emit progress/complete/error events, and return Result.
async fn run_ffmpeg_step(
    args: Vec<String>,
    app: &tauri::AppHandle,
    window_label: &str,
) -> Result<(), AppError> {
    let app_for_blocking = app.clone();
    let window_label_owned = window_label.to_string();
    let result = tauri::async_runtime::spawn_blocking({
        let label = window_label_owned.clone();
        move || run_ffmpeg_blocking(args, Some(&app_for_blocking), Some(&label))
    })
    .await;

    match result {
        Ok(Ok(())) => {
            let _ = app.emit_to(&window_label_owned, "ffmpeg-complete", ());
            Ok(())
        }
        Ok(Err(e)) => {
            let payload = build_error_payload(&e);
            let _ = app.emit_to(&window_label_owned, "ffmpeg-error", payload);
            Err(e)
        }
        Err(join_err) => {
            let e = AppError::from(join_err.to_string());
            let payload = build_error_payload(&e);
            let _ = app.emit_to(&window_label_owned, "ffmpeg-error", payload);
            Err(e)
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
async fn ffmpeg_transcode_to_temp(
    input_path: PathBuf,
    options: TranscodeOptions,
    app: tauri::AppHandle,
    window: tauri::Window,
) -> Result<String, AppError> {
    cleanup_transcode_temp();

    let temp = TempFileManager::default();
    let output_path = temp
        .create("transcode-output.mp4", None)
        .map_err(AppError::from)?;
    let output_str = output_path.to_string_lossy().to_string();

    let args = build_ffmpeg_command(&input_path.to_string_lossy(), &output_str, &options);

    run_ffmpeg_step(args, &app, window.label()).await?;
    set_transcode_temp(Some(output_path));
    Ok(output_str)
}

#[tauri::command(rename_all = "camelCase")]
fn move_compressed_file(source: PathBuf, dest: PathBuf) -> Result<(), AppError> {
    match fs::rename(&source, &dest) {
        Ok(()) => Ok(()),
        Err(e) => {
            #[cfg(unix)]
            if e.raw_os_error() == Some(18) {
                // EXDEV: cross-device link
                fs::copy(&source, &dest)?;
                fs::remove_file(&source)?;
                return Ok(());
            }
            #[cfg(windows)]
            if e.raw_os_error() == Some(17) {
                // ERROR_NOT_SAME_DEVICE
                fs::copy(&source, &dest)?;
                fs::remove_file(&source)?;
                return Ok(());
            }
            Err(e.into())
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
fn cleanup_temp_file(path: PathBuf) -> Result<(), AppError> {
    let _ = fs::remove_file(&path);
    cleanup_transcode_temp();
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
async fn ffmpeg_preview(
    input_path: PathBuf,
    options: TranscodeOptions,
    app: tauri::AppHandle,
    window: tauri::Window,
) -> Result<PreviewResult, AppError> {
    cleanup_previous_preview_paths();

    let temp = TempFileManager::default();
    let preview_duration = options.preview_duration.unwrap_or(3) as f64;
    let original_path = temp.create("preview-original.mp4", None).map_err(AppError::from)?;
    let output_path = temp.create("preview-output.mp4", None).map_err(AppError::from)?;

    let extract_args = vec![
        "-threads".to_string(),
        "0".to_string(),
        "-ss".to_string(),
        "0".to_string(),
        "-t".to_string(),
        preview_duration.to_string(),
        "-i".to_string(),
        input_path.to_string_lossy().to_string(),
        "-c".to_string(),
        "copy".to_string(),
        original_path.to_string_lossy().to_string(),
    ];

    run_ffmpeg_step(extract_args, &app, window.label()).await?;

    let transcode_args = build_ffmpeg_command(
        &original_path.to_string_lossy(),
        &output_path.to_string_lossy(),
        &options,
    );

    run_ffmpeg_step(transcode_args, &app, window.label()).await?;

    let input_size = fs::metadata(&input_path).map_err(AppError::from)?.len();
    let compressed_size = fs::metadata(&output_path).map_err(AppError::from)?.len();
    let original_size = fs::metadata(&original_path).map_err(AppError::from)?.len();
    let ratio = compressed_size as f64 / original_size as f64;
    let estimated_size = (input_size as f64 * ratio) as u64;

    store_preview_paths_for_cleanup(original_path.clone(), output_path.clone());

    Ok(PreviewResult {
        original_path: original_path.to_string_lossy().to_string(),
        compressed_path: output_path.to_string_lossy().to_string(),
        estimated_size,
    })
}

#[tauri::command(rename_all = "camelCase")]
fn get_file_size(path: PathBuf) -> Result<u64, AppError> {
    fs::metadata(&path).map(|m| m.len()).map_err(Into::into)
}

#[tauri::command]
fn ffmpeg_terminate() {
    terminate_all_ffmpeg();
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewResult {
    original_path: String,
    compressed_path: String,
    estimated_size: u64,
}

#[cfg(test)]
mod test_util;

#[cfg(test)]
mod commands_tests;

#[cfg(test)]
mod integration_tests;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .invoke_handler(tauri::generate_handler![
            ffmpeg_transcode_to_temp,
            ffmpeg_preview,
            ffmpeg_terminate,
            get_file_size,
            move_compressed_file,
            cleanup_temp_file,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            cleanup_transcode_temp();
        }
    });
}
