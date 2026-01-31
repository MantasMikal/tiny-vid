mod error;
mod ffmpeg;

use error::AppError;
use tauri::Emitter;
use ffmpeg::{
    build_ffmpeg_command, cleanup_previous_preview_paths, cleanup_transcode_temp, parse_ffmpeg_error,
    run_ffmpeg_blocking, set_transcode_temp, store_preview_paths_for_cleanup, terminate_all_ffmpeg,
    TempFileManager, TranscodeOptions,
};
#[cfg(test)]
use ffmpeg::verify_video;
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
            let (stderr, code) = match &e {
                AppError::FfmpegFailed { code, stderr } => (stderr.clone(), Some(*code)),
                _ => (e.to_string(), None),
            };
            let payload = parse_ffmpeg_error(&stderr, code);
            let _ = app.emit_to(&window_label_owned, "ffmpeg-error", payload);
            Err(e)
        }
        Err(join_err) => {
            let e = AppError::from(join_err.to_string());
            let (stderr, code) = match &e {
                AppError::FfmpegFailed { code, stderr } => (stderr.clone(), Some(*code)),
                _ => (e.to_string(), None),
            };
            let payload = parse_ffmpeg_error(&stderr, code);
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

    let args = build_ffmpeg_command(input_path.as_os_str().to_string_lossy().as_ref(), &output_str, &options);

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
struct PreviewResult {
    original_path: String,
    compressed_path: String,
    estimated_size: u64,
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::fs;
    use tauri::test::{mock_builder, mock_context, noop_assets, INVOKE_KEY};
    use tauri::webview::InvokeRequest;
    use tauri::ipc::{CallbackFn, InvokeBody};

    fn create_test_app() -> tauri::App<tauri::test::MockRuntime> {
        mock_builder()
            .invoke_handler(tauri::generate_handler![
                get_file_size,
                ffmpeg_terminate,
                move_compressed_file,
                cleanup_temp_file,
            ])
            .build(mock_context(noop_assets()))
            .expect("failed to build test app")
    }

    fn invoke_request(cmd: &str, body: InvokeBody) -> InvokeRequest {
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body,
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        }
    }

    #[test]
    fn get_file_size_returns_size() {
        let app = create_test_app();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("failed to create window");

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("testfile");
        fs::write(&path, b"hello").unwrap();

        let body = InvokeBody::from(serde_json::json!({ "path": path.to_string_lossy() }));
        let res = tauri::test::get_ipc_response(&window, invoke_request("get_file_size", body));
        assert!(res.is_ok(), "get_file_size failed: {:?}", res.err());
        let body = res.unwrap();
        let size: u64 = body.deserialize().unwrap();
        assert_eq!(size, 5);
    }

    #[test]
    fn get_file_size_nonexistent_returns_error() {
        let app = create_test_app();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("failed to create window");

        let body = InvokeBody::from(serde_json::json!({
            "path": "/nonexistent/path/that/does/not/exist"
        }));
        let res = tauri::test::get_ipc_response(&window, invoke_request("get_file_size", body));
        assert!(res.is_err());
    }

    #[test]
    fn ffmpeg_terminate_does_not_panic() {
        let app = create_test_app();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("failed to create window");

        let body = InvokeBody::default();
        let _ = tauri::test::get_ipc_response(&window, invoke_request("ffmpeg_terminate", body));
    }

    #[test]
    fn move_compressed_file_renames() {
        let app = create_test_app();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("failed to create window");

        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.mp4");
        let dest = dir.path().join("dest.mp4");
        fs::write(&source, b"video data").unwrap();

        let body = InvokeBody::from(serde_json::json!({
            "source": source.to_string_lossy(),
            "dest": dest.to_string_lossy()
        }));
        let res = tauri::test::get_ipc_response(&window, invoke_request("move_compressed_file", body));
        assert!(res.is_ok(), "move_compressed_file failed: {:?}", res.err());
        assert!(!source.exists());
        assert!(dest.exists());
        assert_eq!(fs::read(&dest).unwrap(), b"video data");
    }

    fn run_transcode_integration(
        options: TranscodeOptions,
        duration_secs: f32,
        skip_if_encoder_missing: bool,
    ) {
        use std::fs;
        use std::path::PathBuf;
        use std::process::Command;

        let ffmpeg = {
            if let Ok(p) = std::env::var("FFMPEG_PATH") {
                let path = PathBuf::from(p);
                if path.exists() {
                    Some(path)
                } else {
                    None
                }
            } else {
                None
            }
        }
        .or_else(|| {
            let cmd = if cfg!(windows) { "where" } else { "which" };
            let output = Command::new(cmd).arg("ffmpeg").output().ok()?;
            if output.status.success() {
                let first = std::str::from_utf8(&output.stdout)
                    .ok()?
                    .lines()
                    .next()?
                    .trim();
                if !first.is_empty() {
                    return Some(PathBuf::from(first));
                }
            }
            None
        });

        let ffmpeg = ffmpeg.expect("FFmpeg not found; set FFMPEG_PATH or add to PATH");
        std::env::set_var("FFMPEG_PATH", ffmpeg.to_string_lossy().as_ref());

        let dir = tempfile::tempdir().unwrap();
        let input_path = dir.path().join("input.mp4");
        let output_path = dir.path().join("output.mp4");

        let duration_arg = format!("{}", duration_secs);
        let status = Command::new(&ffmpeg)
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                &format!("testsrc=duration={}:size=320x240:rate=30", duration_arg),
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                input_path.to_str().unwrap(),
            ])
            .status()
            .expect("failed to create test video");
        assert!(status.success(), "ffmpeg failed to create test video");

        let args = build_ffmpeg_command(
            input_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
            &options,
        );

        let result = run_ffmpeg_blocking(args, None, None);
        if let Err(ref e) = result {
            if skip_if_encoder_missing {
                let stderr = format!("{}", e);
                if stderr.to_lowercase().contains("unknown encoder")
                    || stderr.to_lowercase().contains("encoder not found")
                {
                    return;
                }
            }
        }
        assert!(result.is_ok(), "run_ffmpeg_blocking failed: {:?}", result.err());
        assert!(output_path.exists());
        assert!(fs::metadata(&output_path).unwrap().len() > 0);

        let verify_result = verify_video(&output_path, options.codec.as_deref());
        assert!(
            verify_result.is_ok(),
            "Encoded video failed verification (corrupted): {}",
            verify_result.unwrap_err()
        );
    }

    #[test]
    #[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_transcode_integration -- --ignored"]
    fn ffmpeg_transcode_integration() {
        let option_sets: Vec<(TranscodeOptions, f32, bool)> = vec![
            (
                TranscodeOptions {
                    codec: Some("libx264".to_string()),
                    quality: Some(75),
                    max_bitrate: None,
                    scale: Some(1.0),
                    fps: Some(30),
                    remove_audio: Some(true),
                    preset: Some("ultrafast".to_string()),
                    tune: None,
                    preview_duration: None,
                },
                1.0,
                false,
            ),
            (
                TranscodeOptions {
                    codec: Some("libx264".to_string()),
                    quality: Some(75),
                    max_bitrate: None,
                    scale: Some(1.0),
                    fps: Some(30),
                    remove_audio: Some(false),
                    preset: Some("ultrafast".to_string()),
                    tune: None,
                    preview_duration: None,
                },
                1.0,
                false,
            ),
            (
                TranscodeOptions {
                    codec: Some("libx264".to_string()),
                    quality: Some(75),
                    max_bitrate: None,
                    scale: Some(0.5),
                    fps: Some(30),
                    remove_audio: Some(true),
                    preset: Some("ultrafast".to_string()),
                    tune: None,
                    preview_duration: None,
                },
                1.0,
                false,
            ),
            (
                TranscodeOptions {
                    codec: Some("libx264".to_string()),
                    quality: Some(75),
                    max_bitrate: Some(1000),
                    scale: Some(1.0),
                    fps: Some(30),
                    remove_audio: Some(true),
                    preset: Some("ultrafast".to_string()),
                    tune: None,
                    preview_duration: None,
                },
                1.0,
                false,
            ),
            (
                TranscodeOptions {
                    codec: Some("libx264".to_string()),
                    quality: Some(75),
                    max_bitrate: None,
                    scale: Some(1.0),
                    fps: Some(30),
                    remove_audio: Some(true),
                    preset: Some("ultrafast".to_string()),
                    tune: Some("film".to_string()),
                    preview_duration: None,
                },
                1.0,
                false,
            ),
            (
                TranscodeOptions {
                    codec: Some("libx265".to_string()),
                    quality: Some(75),
                    max_bitrate: None,
                    scale: Some(1.0),
                    fps: Some(30),
                    remove_audio: Some(true),
                    preset: Some("ultrafast".to_string()),
                    tune: None,
                    preview_duration: None,
                },
                1.0,
                false,
            ),
            (
                TranscodeOptions {
                    codec: Some("libsvtav1".to_string()),
                    quality: Some(75),
                    max_bitrate: None,
                    scale: Some(1.0),
                    fps: Some(30),
                    remove_audio: Some(true),
                    preset: Some("ultrafast".to_string()),
                    tune: None,
                    preview_duration: None,
                },
                1.0,
                false,
            ),
            (
                TranscodeOptions {
                    codec: Some("libsvtav1".to_string()),
                    quality: Some(75),
                    max_bitrate: Some(1000),
                    scale: Some(1.0),
                    fps: Some(30),
                    remove_audio: Some(true),
                    preset: Some("ultrafast".to_string()),
                    tune: None,
                    preview_duration: None,
                },
                1.0,
                false,
            ),
            (
                TranscodeOptions {
                    codec: Some("libsvtav1".to_string()),
                    quality: Some(75),
                    max_bitrate: None,
                    scale: Some(0.5),
                    fps: Some(30),
                    remove_audio: Some(true),
                    preset: Some("ultrafast".to_string()),
                    tune: None,
                    preview_duration: None,
                },
                1.0,
                false,
            ),
            (
                TranscodeOptions {
                    codec: Some("libx264".to_string()),
                    quality: Some(75),
                    max_bitrate: None,
                    scale: Some(1.0),
                    fps: Some(24),
                    remove_audio: Some(true),
                    preset: Some("ultrafast".to_string()),
                    tune: None,
                    preview_duration: None,
                },
                1.0,
                false,
            ),
        ];

        for (options, duration_secs, skip_if_encoder_missing) in option_sets {
            run_transcode_integration(options, duration_secs, skip_if_encoder_missing);
        }
    }

    #[test]
    fn cleanup_temp_file_removes_file() {
        let app = create_test_app();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("failed to create window");

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("temp.mp4");
        fs::write(&path, b"temp").unwrap();

        let body = InvokeBody::from(serde_json::json!({ "path": path.to_string_lossy() }));
        let res = tauri::test::get_ipc_response(&window, invoke_request("cleanup_temp_file", body));
        assert!(res.is_ok());
        assert!(!path.exists());
    }
}



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
