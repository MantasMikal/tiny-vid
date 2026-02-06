//! Shared test helpers for Tauri command tests.
//!
//! Layout: unit tests live in each module (error, ffmpeg/*); command tests
//! use this module and live in `commands_tests.rs`; the FFmpeg integration
//! test lives in `integration_tests.rs`. Run ignored tests with:
//! `cargo test -- --ignored`.

use std::path::{Path, PathBuf};

use crate::ffmpeg::TranscodeOptions;

/// Build TranscodeOptions with overrides.
pub fn opts_with(overrides: impl FnOnce(&mut TranscodeOptions)) -> TranscodeOptions {
    let mut o = TranscodeOptions::default();
    overrides(&mut o);
    o
}

/// Preview options for integration tests. Preset varies by lgpl feature.
pub fn preview_options(preview_duration: u32) -> TranscodeOptions {
    opts_with(|o| {
        o.codec = Some(if cfg!(feature = "lgpl") {
            "h264_videotoolbox".into()
        } else {
            "libx264".into()
        });
        o.remove_audio = Some(true);
        o.preset = Some("ultrafast".into());
        o.preview_duration = Some(preview_duration);
    })
}
use std::process::{Command, Stdio};

use crate::commands;
use crate::AppState;
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
/// Uses libx264 or h264_videotoolbox depending on lgpl feature.
pub fn create_test_video(
    ffmpeg: &Path,
    output_path: &Path,
    duration_secs: f32,
) -> std::io::Result<std::process::ExitStatus> {
    let duration_arg = format!("{}", duration_secs);
    #[cfg(not(feature = "lgpl"))]
    {
        Command::new(ffmpeg)
            .args([
                "-loglevel",
                "error",
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
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    }
    #[cfg(feature = "lgpl")]
    {
        Command::new(ffmpeg)
            .args([
                "-loglevel",
                "error",
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
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    }
}

/// Creates a test video with multiple audio tracks using lavfi testsrc + sine.
/// `audio_track_count`: number of separate audio streams (e.g. 2 = two stereo tracks).
/// Uses libx264 (standalone) or h264_videotoolbox (lgpl) for video.
pub fn create_test_video_with_multi_audio(
    ffmpeg: &Path,
    output_path: &Path,
    duration_secs: f32,
    audio_track_count: u32,
) -> std::io::Result<std::process::ExitStatus> {
    if audio_track_count == 0 {
        return create_test_video(ffmpeg, output_path, duration_secs);
    }

    let duration_arg = format!("{}", duration_secs);
    let mut args = vec![
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        "-f".to_string(),
        "lavfi".to_string(),
        "-i".to_string(),
        format!("testsrc=duration={}:size=320x240:rate=30", duration_arg),
    ];

    for i in 0..audio_track_count {
        let freq = 440 + (i as i32) * 220;
        args.push("-f".to_string());
        args.push("lavfi".to_string());
        args.push("-i".to_string());
        args.push(format!("sine=frequency={}:duration={}", freq, duration_arg));
    }

    args.push("-map".to_string());
    args.push("0:v".to_string());
    for i in 0..audio_track_count {
        args.push("-map".to_string());
        args.push(format!("{}:a", i + 1));
    }
    args.push("-c:v".to_string());
    #[cfg(not(feature = "lgpl"))]
    args.push("libx264".to_string());
    #[cfg(not(feature = "lgpl"))]
    {
        args.push("-pix_fmt".to_string());
        args.push("yuv420p".to_string());
    }
    #[cfg(feature = "lgpl")]
    {
        args.push("h264_videotoolbox".to_string());
        args.push("-allow_sw".to_string());
        args.push("1".to_string());
        args.push("-q:v".to_string());
        args.push("25".to_string());
    }
    args.push("-c:a".to_string());
    args.push("aac".to_string());
    args.push("-shortest".to_string());
    args.push(output_path.to_str().unwrap().to_string());

    Command::new(ffmpeg)
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
}

pub fn create_test_app() -> tauri::App<tauri::test::MockRuntime> {
    mock_builder()
        .invoke_handler(tauri::generate_handler![
            commands::get_file_size,
            commands::get_video_metadata,
            commands::get_build_variant,
            commands::ffmpeg_terminate,
            commands::move_compressed_file,
            commands::cleanup_temp_file,
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
