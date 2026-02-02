//! Shared test helpers for Tauri command tests.
//!
//! Layout: unit tests live in each module (error, ffmpeg/*); command tests
//! use this module and live in `commands_tests.rs`; the FFmpeg integration
//! test lives in `integration_tests.rs`. Run ignored tests with:
//! `cargo test -- --ignored`.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{
    cleanup_temp_file, ffmpeg_terminate, get_build_variant, get_file_size, get_pending_opened_files,
    get_video_metadata, move_compressed_file, AppState,
};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{mock_builder, mock_context, noop_assets, INVOKE_KEY};
use tauri::webview::InvokeRequest;

/// Finds FFmpeg via FFMPEG_PATH env or `which`/`where`, sets FFMPEG_PATH for the process, and returns its path.
/// Use in tests that need a real FFmpeg binary.
pub fn find_ffmpeg_and_set_env() -> PathBuf {
    let path = std::env::var("FFMPEG_PATH")
        .ok()
        .map(PathBuf::from)
        .filter(|p| p.exists())
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
        })
        .expect("FFmpeg not found; set FFMPEG_PATH or add to PATH");
    // SAFETY: Single-threaded test; no other threads access env vars during test
    unsafe {
        std::env::set_var("FFMPEG_PATH", path.to_string_lossy().as_ref());
    }
    path
}

/// Creates a short test video at `output_path` using lavfi testsrc. Duration in seconds.
/// Uses libx264 or h264_videotoolbox depending on lgpl-macos feature.
pub fn create_test_video(
    ffmpeg: &Path,
    output_path: &Path,
    duration_secs: f32,
) -> std::io::Result<std::process::ExitStatus> {
    let duration_arg = format!("{}", duration_secs);
    #[cfg(not(feature = "lgpl-macos"))]
    {
        Command::new(ffmpeg)
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
                output_path.to_str().unwrap(),
            ])
            .status()
    }
    #[cfg(feature = "lgpl-macos")]
    {
        Command::new(ffmpeg)
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                &format!("testsrc=duration={}:size=320x240:rate=30", duration_arg),
                "-c:v",
                "h264_videotoolbox",
                "-allow_sw",
                "1",
                "-q:v",
                "25",
                output_path.to_str().unwrap(),
            ])
            .status()
    }
}

pub fn create_test_app() -> tauri::App<tauri::test::MockRuntime> {
    mock_builder()
        .invoke_handler(tauri::generate_handler![
            get_file_size,
            get_video_metadata,
            get_build_variant,
            ffmpeg_terminate,
            move_compressed_file,
            cleanup_temp_file,
        ])
        .build(mock_context(noop_assets()))
        .expect("failed to build test app")
}

/// Creates a test app with file association support (AppState + get_pending_opened_files).
/// When `pending` is `None`, uses empty buffer; when `Some(paths)`, pre-populates the buffer.
pub fn create_test_app_with_file_assoc(
    pending: Option<Vec<PathBuf>>,
) -> tauri::App<tauri::test::MockRuntime> {
    let state = match pending {
        None => AppState::default(),
        Some(paths) => AppState::with_pending(paths),
    };
    mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_file_size,
            get_video_metadata,
            get_build_variant,
            ffmpeg_terminate,
            move_compressed_file,
            cleanup_temp_file,
            get_pending_opened_files,
        ])
        .build(mock_context(noop_assets()))
        .expect("failed to build test app")
}

pub fn invoke_request(cmd: &str, body: InvokeBody) -> InvokeRequest {
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
