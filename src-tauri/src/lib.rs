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
    duration_secs: Option<f64>,
) -> Result<(), AppError> {
    let app_for_blocking = app.clone();
    let window_label_owned = window_label.to_string();
    let result = tauri::async_runtime::spawn_blocking({
        let label = window_label_owned.clone();
        move || run_ffmpeg_blocking(args, Some(&app_for_blocking), Some(&label), duration_secs, None)
    })
    .await;

    match result {
        Ok(Ok(())) => {
            log::trace!(
                target: "tiny_vid::commands",
                "emitting ffmpeg-complete to window={}",
                window_label_owned
            );
            let _ = app.emit_to(&window_label_owned, "ffmpeg-complete", ());
            Ok(())
        }
        Ok(Err(e)) => {
            log::error!(
                target: "tiny_vid::commands",
                "ffmpeg-error: {}",
                e
            );
            let payload = build_error_payload(&e);
            let _ = app.emit_to(&window_label_owned, "ffmpeg-error", payload);
            Err(e)
        }
        Err(join_err) => {
            let e = AppError::from(join_err.to_string());
            log::error!(
                target: "tiny_vid::commands",
                "ffmpeg-error (join): {}",
                e
            );
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
    log::info!(
        target: "tiny_vid::commands",
        "ffmpeg_transcode_to_temp: input={}",
        input_path.display()
    );
    cleanup_transcode_temp();

    let temp = TempFileManager::default();
    let output_path = temp
        .create("transcode-output.mp4", None)
        .map_err(AppError::from)?;
    let output_str = output_path.to_string_lossy().to_string();

    set_transcode_temp(Some(output_path.clone()));

    let args = build_ffmpeg_command(&input_path.to_string_lossy(), &output_str, &options);
    let duration_secs = options.duration_secs;

    match run_ffmpeg_step(args, &app, window.label(), duration_secs).await {
        Ok(()) => {
            log::info!(
                target: "tiny_vid::commands",
                "ffmpeg_transcode_to_temp: complete -> {}",
                output_str
            );
            Ok(output_str)
        }
        Err(e) => {
            cleanup_transcode_temp();
            Err(e)
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
fn move_compressed_file(source: PathBuf, dest: PathBuf) -> Result<(), AppError> {
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
async fn ffmpeg_preview(
    input_path: PathBuf,
    options: TranscodeOptions,
    app: tauri::AppHandle,
    window: tauri::Window,
) -> Result<PreviewResult, AppError> {
    log::info!(
        target: "tiny_vid::commands",
        "ffmpeg_preview: input={}",
        input_path.display()
    );
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

    run_ffmpeg_step(extract_args, &app, window.label(), None).await?;

    let transcode_args = build_ffmpeg_command(
        &original_path.to_string_lossy(),
        &output_path.to_string_lossy(),
        &options,
    );

    run_ffmpeg_step(transcode_args, &app, window.label(), None).await?;

    let input_size = fs::metadata(&input_path).map_err(AppError::from)?.len();
    let compressed_size = fs::metadata(&output_path).map_err(AppError::from)?.len();
    let original_size = fs::metadata(&original_path).map_err(AppError::from)?.len();
    let ratio = compressed_size as f64 / original_size as f64;
    let estimated_size = (input_size as f64 * ratio) as u64;

    store_preview_paths_for_cleanup(original_path.clone(), output_path.clone());

    log::info!(
        target: "tiny_vid::commands",
        "ffmpeg_preview: complete, estimated_size={}",
        estimated_size
    );
    Ok(PreviewResult {
        original_path: original_path.to_string_lossy().to_string(),
        compressed_path: output_path.to_string_lossy().to_string(),
        estimated_size,
    })
}

#[tauri::command(rename_all = "camelCase")]
fn get_file_size(path: PathBuf) -> Result<u64, AppError> {
    log::debug!(
        target: "tiny_vid::commands",
        "get_file_size: path={}",
        path.display()
    );
    fs::metadata(&path).map(|m| m.len()).map_err(Into::into)
}

#[tauri::command]
fn ffmpeg_terminate() {
    log::info!(target: "tiny_vid::commands", "ffmpeg_terminate: terminating all FFmpeg processes");
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

fn build_log_plugin() -> tauri_plugin_log::Builder {
    use tauri_plugin_log::fern::colors::{Color, ColoredLevelConfig};
    use time::macros::format_description;

    let colors = ColoredLevelConfig::default()
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::Cyan)
        .debug(Color::Magenta)
        .trace(Color::BrightBlack);

    let timezone = tauri_plugin_log::TimezoneStrategy::UseLocal;
    let time_fmt = format_description!("[hour]:[minute]:[second]");

    let mut builder = tauri_plugin_log::Builder::new()
        .timezone_strategy(timezone.clone())
        .format(move |out, message, record| {
            let now = timezone.get_now();
            let ts = now
                .format(&time_fmt)
                .unwrap_or_else(|_| "??:??:??".into());
            let target = record
                .target()
                .strip_prefix("tiny_vid_tauri::")
                .or_else(|| record.target().strip_prefix("tiny_vid::"))
                .unwrap_or(record.target());
            out.finish(format_args!(
                "{ts}  {level:5}  {target:5}  {message}",
                ts = ts,
                level = colors.color(record.level()),
                target = target,
                message = message
            ))
        });

    #[cfg(debug_assertions)]
    {
        builder = builder
            .level(log::LevelFilter::Debug)
            .level_for("tiny_vid_tauri", log::LevelFilter::Trace);
    }
    #[cfg(not(debug_assertions))]
    {
        builder = builder.level(log::LevelFilter::Info);
    }
    builder
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(build_log_plugin().build())
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
            log::info!(target: "tiny_vid::commands", "app exit requested, cleaning up");
            cleanup_transcode_temp();
        }
    });
}
