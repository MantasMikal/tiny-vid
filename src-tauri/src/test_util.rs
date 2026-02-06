//! Shared test helpers for Tauri command tests.
//!
//! Layout: unit tests live in each module (error, ffmpeg/*); command tests
//! use this module and live in `commands_tests.rs`; the FFmpeg integration
//! test lives in `integration_tests.rs`. Run ignored tests with:
//! `cargo test -- --ignored`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::ffmpeg::TranscodeOptions;

/// Codec for integration tests based on build (lgpl → h264_videotoolbox, standalone → libx264).
pub fn default_codec() -> String {
    if cfg!(feature = "lgpl") {
        "h264_videotoolbox".into()
    } else {
        "libx264".into()
    }
}

/// Build TranscodeOptions with overrides.
pub fn opts_with(overrides: impl FnOnce(&mut TranscodeOptions)) -> TranscodeOptions {
    let mut o = TranscodeOptions::default();
    overrides(&mut o);
    o
}

/// Preview options for integration tests. Preset varies by lgpl feature.
pub fn preview_options(preview_duration: u32) -> TranscodeOptions {
    opts_with(|o| {
        o.codec = Some(default_codec());
        o.remove_audio = Some(true);
        o.preset = Some("ultrafast".into());
        o.preview_duration = Some(preview_duration);
    })
}

/// Video kind for IntegrationEnv::with_test_video.
pub enum VideoKind {
    Plain,
    MultiAudio(u32),
    Subtitles,
    SubtitlesNoAudio,
}

/// Integration test environment: FFmpeg path, temp dir, and helpers.
pub struct IntegrationEnv {
    pub ffmpeg: PathBuf,
    dir: tempfile::TempDir,
}

impl IntegrationEnv {
    pub fn new() -> Self {
        let ffmpeg = find_ffmpeg_and_set_env();
        let dir = tempfile::tempdir().expect("tempdir");
        Self { ffmpeg, dir }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// Creates a test video at `input_name` and asserts success.
    pub fn with_test_video(&self, input_name: &str, duration_secs: f32, kind: VideoKind) -> PathBuf {
        let output_path = self.path(input_name);
        let status = match kind {
            VideoKind::Plain => create_test_video(&self.ffmpeg, &output_path, duration_secs),
            VideoKind::MultiAudio(n) => {
                create_test_video_with_multi_audio(&self.ffmpeg, &output_path, duration_secs, n)
            }
            VideoKind::Subtitles => {
                create_test_video_with_subtitles(&self.ffmpeg, &output_path, duration_secs)
            }
            VideoKind::SubtitlesNoAudio => {
                create_test_video_with_subtitles_no_audio(&self.ffmpeg, &output_path, duration_secs)
            }
        };
        let status = status.expect("failed to create test video");
        assert!(status.success(), "ffmpeg failed to create test video");
        output_path
    }
}

/// Runs transcode and verifies output. On encoder missing + skip_if_encoder_missing, returns Ok(()).
pub fn run_transcode_and_verify(
    input_path: &Path,
    output_path: &Path,
    options: &TranscodeOptions,
    duration_secs: Option<f64>,
    skip_if_encoder_missing: bool,
) -> Result<(), String> {
    use crate::ffmpeg::{build_ffmpeg_command, run_ffmpeg_blocking, verify_video};

    let args = build_ffmpeg_command(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        options,
        None,
        None,
        None,
    )
    .map_err(|e| e.to_string())?;

    let result = run_ffmpeg_blocking(args, None, None, duration_secs, None, None);

    if let Err(ref e) = result {
        if skip_if_encoder_missing {
            let stderr = format!("{}", e);
            if stderr.to_lowercase().contains("unknown encoder")
                || stderr.to_lowercase().contains("encoder not found")
            {
                return Ok(());
            }
        }
        return Err(format!("run_ffmpeg_blocking failed: {:?}", e));
    }

    if !output_path.exists() {
        return Err("output path does not exist".into());
    }
    if fs::metadata(output_path).map_err(|e| e.to_string())?.len() == 0 {
        return Err("output file is empty".into());
    }

    verify_video(output_path, options.codec.as_deref())
        .map_err(|e| format!("Encoded video failed verification: {}", e))
}


/// Runs preview and asserts original_path and compressed_path exist.
pub fn run_preview_and_assert_exists(
    input_path: &Path,
    opts: &TranscodeOptions,
    region: Option<f64>,
) -> crate::preview::PreviewResult {
    let result = tauri::async_runtime::block_on(crate::preview::run_preview_core(
        input_path,
        opts,
        region,
        None,
        None,
        None,
        None,
    ))
    .expect("run_preview_core");
    assert!(Path::new(&result.original_path).exists());
    assert!(Path::new(&result.compressed_path).exists());
    result
}

/// Runs preview with estimate and asserts paths exist.
pub fn run_preview_with_estimate_and_assert(
    input_path: &Path,
    opts: &TranscodeOptions,
    region: Option<f64>,
) -> crate::preview::PreviewWithEstimateResult {
    let result =
        tauri::async_runtime::block_on(crate::preview::run_preview_with_estimate_core(
            input_path,
            opts,
            region,
            None,
        ))
        .expect("run_preview_with_estimate_core");
    assert!(Path::new(&result.preview.original_path).exists());
    assert!(Path::new(&result.preview.compressed_path).exists());
    result
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

/// Creates a short test video at `output_path` using lavfi testsrc.
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
    {
        args.push("libx264".to_string());
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

/// Creates a test video with a subtitle stream using lavfi testsrc + sine + SRT.
pub fn create_test_video_with_subtitles(
    ffmpeg: &Path,
    output_path: &Path,
    duration_secs: f32,
) -> std::io::Result<std::process::ExitStatus> {
    let srt_path = output_path
        .parent()
        .unwrap_or_else(|| output_path.as_ref())
        .join("test_subs.srt");
    let srt_content = format!(
        "1\n00:00:00,000 --> 00:00:{:02},000\nTest subtitle\n",
        duration_secs.ceil() as u32
    );
    fs::write(&srt_path, srt_content)?;

    let duration_arg = format!("{}", duration_secs);
    let mut args = vec![
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        "-f".to_string(),
        "lavfi".to_string(),
        "-i".to_string(),
        format!("testsrc=duration={}:size=320x240:rate=30", duration_arg),
        "-f".to_string(),
        "lavfi".to_string(),
        "-i".to_string(),
        format!("sine=frequency=440:duration={}", duration_arg),
        "-f".to_string(),
        "srt".to_string(),
        "-i".to_string(),
        srt_path.to_string_lossy().to_string(),
        "-map".to_string(),
        "0:v".to_string(),
        "-map".to_string(),
        "1:a".to_string(),
        "-map".to_string(),
        "2:s".to_string(),
        "-c:v".to_string(),
    ];
    #[cfg(not(feature = "lgpl"))]
    {
        args.push("libx264".to_string());
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
    args.push("-c:s".to_string());
    args.push("mov_text".to_string());
    args.push("-shortest".to_string());
    args.push(output_path.to_str().unwrap().to_string());

    let result = Command::new(ffmpeg)
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = fs::remove_file(srt_path);
    result
}

/// Creates a test video with a subtitle stream and no audio.
pub fn create_test_video_with_subtitles_no_audio(
    ffmpeg: &Path,
    output_path: &Path,
    duration_secs: f32,
) -> std::io::Result<std::process::ExitStatus> {
    let srt_path = output_path
        .parent()
        .unwrap_or_else(|| output_path.as_ref())
        .join("test_subs.srt");
    let srt_content = format!(
        "1\n00:00:00,000 --> 00:00:{:02},000\nTest subtitle\n",
        duration_secs.ceil() as u32
    );
    fs::write(&srt_path, srt_content)?;

    let duration_arg = format!("{}", duration_secs);
    let mut args = vec![
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        "-f".to_string(),
        "lavfi".to_string(),
        "-i".to_string(),
        format!("testsrc=duration={}:size=320x240:rate=30", duration_arg),
        "-f".to_string(),
        "srt".to_string(),
        "-i".to_string(),
        srt_path.to_string_lossy().to_string(),
        "-map".to_string(),
        "0:v".to_string(),
        "-map".to_string(),
        "1:s".to_string(),
        "-c:v".to_string(),
    ];
    #[cfg(not(feature = "lgpl"))]
    {
        args.push("libx264".to_string());
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
    args.push("-c:s".to_string());
    args.push("mov_text".to_string());
    args.push("-shortest".to_string());
    args.push(output_path.to_str().unwrap().to_string());

    let result = Command::new(ffmpeg)
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = fs::remove_file(srt_path);
    result
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
