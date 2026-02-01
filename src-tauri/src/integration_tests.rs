//! FFmpeg integration test. Requires FFmpeg on the system; run with:
//! `cargo test ffmpeg_transcode_integration -- --ignored`
//! `cargo test ffmpeg_progress_emission_integration -- --ignored`

use crate::ffmpeg::{
    build_ffmpeg_command, cleanup_transcode_temp, run_ffmpeg_blocking, set_transcode_temp,
    verify_video, TempFileManager, TranscodeOptions,
};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration as StdDuration;

fn run_transcode_integration(
    options: TranscodeOptions,
    duration_secs: f32,
    skip_if_encoder_missing: bool,
) {
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
    // SAFETY: Single-threaded test; no other threads access env vars during test
    unsafe {
        std::env::set_var("FFMPEG_PATH", ffmpeg.to_string_lossy().as_ref());
    }

    let dir = tempfile::tempdir().unwrap();
    let input_path = dir.path().join("input.mp4");
    let output_path = dir.path().join("output.mp4");

    let duration_arg = format!("{}", duration_secs);
    let status = {
        #[cfg(not(feature = "lgpl-macos"))]
        {
            Command::new(&ffmpeg)
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
        }
        #[cfg(feature = "lgpl-macos")]
        {
            Command::new(&ffmpeg)
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
                    input_path.to_str().unwrap(),
                ])
                .status()
        }
    }
    .expect("failed to create test video");
    assert!(status.success(), "ffmpeg failed to create test video");

    let args = build_ffmpeg_command(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        &options,
    )
    .expect("build_ffmpeg_command");

    let result = run_ffmpeg_blocking(args, None, None, None, None);
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
    assert!(
        result.is_ok(),
        "run_ffmpeg_blocking failed: {:?}",
        result.err()
    );
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
    let option_sets: Vec<(TranscodeOptions, f32, bool)> = {
        #[cfg(not(feature = "lgpl-macos"))]
        {
            vec![
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
                        output_format: None,
                        preview_duration: None,
                        duration_secs: None,
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
                        output_format: None,
                        preview_duration: None,
                        duration_secs: None,
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
                        output_format: None,
                        preview_duration: None,
                        duration_secs: None,
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
                        output_format: None,
                        preview_duration: None,
                        duration_secs: None,
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
                        output_format: None,
                        preview_duration: None,
                        duration_secs: None,
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
                        output_format: None,
                        preview_duration: None,
                        duration_secs: None,
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
                        output_format: None,
                        preview_duration: None,
                        duration_secs: None,
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
                        output_format: None,
                        preview_duration: None,
                        duration_secs: None,
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
                        output_format: None,
                        preview_duration: None,
                        duration_secs: None,
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
                        output_format: None,
                        preview_duration: None,
                        duration_secs: None,
                    },
                    1.0,
                    false,
                ),
            ]
        }
        #[cfg(all(feature = "lgpl-macos", not(target_os = "macos")))]
        {
            vec![] // lgpl-macos is macOS-only; skip on other platforms
        }
        #[cfg(all(feature = "lgpl-macos", target_os = "macos"))]
        {
            vec![
                (
                    TranscodeOptions {
                        codec: Some("h264_videotoolbox".to_string()),
                        quality: Some(75),
                        max_bitrate: None,
                        scale: Some(1.0),
                        fps: Some(30),
                        remove_audio: Some(true),
                        preset: Some("fast".to_string()),
                        tune: None,
                        output_format: Some("mp4".to_string()),
                        preview_duration: None,
                        duration_secs: None,
                    },
                    1.0,
                    true,
                ),
                (
                    TranscodeOptions {
                        codec: Some("h264_videotoolbox".to_string()),
                        quality: Some(0),
                        max_bitrate: None,
                        scale: Some(1.0),
                        fps: Some(30),
                        remove_audio: Some(true),
                        preset: Some("fast".to_string()),
                        tune: None,
                        output_format: Some("mp4".to_string()),
                        preview_duration: None,
                        duration_secs: None,
                    },
                    1.0,
                    true,
                ),
                (
                    TranscodeOptions {
                        codec: Some("h264_videotoolbox".to_string()),
                        quality: Some(100),
                        max_bitrate: None,
                        scale: Some(1.0),
                        fps: Some(30),
                        remove_audio: Some(false),
                        preset: Some("fast".to_string()),
                        tune: None,
                        output_format: Some("mp4".to_string()),
                        preview_duration: None,
                        duration_secs: None,
                    },
                    1.0,
                    true,
                ),
                (
                    TranscodeOptions {
                        codec: Some("h264_videotoolbox".to_string()),
                        quality: Some(75),
                        max_bitrate: None,
                        scale: Some(0.5),
                        fps: Some(30),
                        remove_audio: Some(true),
                        preset: Some("fast".to_string()),
                        tune: None,
                        output_format: Some("mp4".to_string()),
                        preview_duration: None,
                        duration_secs: None,
                    },
                    1.0,
                    true,
                ),
                (
                    TranscodeOptions {
                        codec: Some("h264_videotoolbox".to_string()),
                        quality: Some(75),
                        max_bitrate: Some(1000),
                        scale: Some(1.0),
                        fps: Some(30),
                        remove_audio: Some(true),
                        preset: Some("fast".to_string()),
                        tune: None,
                        output_format: Some("mp4".to_string()),
                        preview_duration: None,
                        duration_secs: None,
                    },
                    1.0,
                    true,
                ),
                (
                    TranscodeOptions {
                        codec: Some("hevc_videotoolbox".to_string()),
                        quality: Some(75),
                        max_bitrate: None,
                        scale: Some(1.0),
                        fps: Some(30),
                        remove_audio: Some(true),
                        preset: Some("fast".to_string()),
                        tune: None,
                        output_format: Some("mp4".to_string()),
                        preview_duration: None,
                        duration_secs: None,
                    },
                    1.0,
                    true,
                ),
                (
                    TranscodeOptions {
                        codec: Some("hevc_videotoolbox".to_string()),
                        quality: Some(50),
                        max_bitrate: None,
                        scale: Some(0.5),
                        fps: Some(30),
                        remove_audio: Some(true),
                        preset: Some("fast".to_string()),
                        tune: None,
                        output_format: Some("mp4".to_string()),
                        preview_duration: None,
                        duration_secs: None,
                    },
                    1.0,
                    true,
                ),
            ]
        }
    };

    for (options, duration_secs, skip_if_encoder_missing) in option_sets {
        run_transcode_integration(options, duration_secs, skip_if_encoder_missing);
    }
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_progress_emission_integration -- --ignored"]
fn ffmpeg_progress_emission_integration() {
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
    // SAFETY: Single-threaded test; no other threads access env vars during test
    unsafe {
        std::env::set_var("FFMPEG_PATH", ffmpeg.to_string_lossy().as_ref());
    }

    let dir = tempfile::tempdir().unwrap();
    let input_path = dir.path().join("input.mp4");
    let output_path = dir.path().join("output.mp4");

    // 2 seconds - long enough for multiple progress updates
    let duration_secs = 2.0_f32;
    let status = {
        #[cfg(not(feature = "lgpl-macos"))]
        {
            Command::new(&ffmpeg)
                .args([
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    &format!("testsrc=duration={}:size=320x240:rate=30", duration_secs),
                    "-c:v",
                    "libx264",
                    "-pix_fmt",
                    "yuv420p",
                    input_path.to_str().unwrap(),
                ])
                .status()
        }
        #[cfg(feature = "lgpl-macos")]
        {
            Command::new(&ffmpeg)
                .args([
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    &format!("testsrc=duration={}:size=320x240:rate=30", duration_secs),
                    "-c:v",
                    "h264_videotoolbox",
                    "-allow_sw",
                    "1",
                    "-q:v",
                    "25",
                    input_path.to_str().unwrap(),
                ])
                .status()
        }
    }
    .expect("failed to create test video");
    assert!(status.success(), "ffmpeg failed to create test video");

    let options = TranscodeOptions {
        codec: Some(if cfg!(feature = "lgpl-macos") {
            "h264_videotoolbox".to_string()
        } else {
            "libx264".to_string()
        }),
        quality: Some(75),
        max_bitrate: None,
        scale: Some(1.0),
        fps: Some(30),
        remove_audio: Some(true),
        preset: Some("ultrafast".to_string()),
        tune: None,
        output_format: None,
        preview_duration: None,
        duration_secs: None,
    };

    let args = build_ffmpeg_command(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        &options,
    )
    .expect("build_ffmpeg_command");

    let progress_collector: Arc<Mutex<Vec<f64>>> = Arc::new(Mutex::new(Vec::new()));
    let result = run_ffmpeg_blocking(
        args,
        None,
        None,
        Some(duration_secs as f64),
        Some(Arc::clone(&progress_collector)),
    );

    assert!(
        result.is_ok(),
        "run_ffmpeg_blocking failed: {:?}",
        result.err()
    );

    let progress_values = progress_collector
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();

    assert!(
        !progress_values.is_empty(),
        "expected at least one progress value"
    );
    assert!(
        progress_values.last().copied().unwrap_or(0.0) >= 0.98,
        "expected progress to reach ~1.0, got {:?}",
        progress_values.last()
    );
    // Values should increase monotonically (or be single value for very fast transcodes)
    for w in progress_values.windows(2) {
        assert!(w[1] >= w[0], "progress should increase: {:?}", progress_values);
    }
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_cancel_cleanup_integration -- --ignored"]
fn ffmpeg_cancel_cleanup_integration() {
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
    // SAFETY: Single-threaded test; no other threads access env vars during test
    unsafe {
        std::env::set_var("FFMPEG_PATH", ffmpeg.to_string_lossy().as_ref());
    }

    let dir = tempfile::tempdir().unwrap();
    let input_path = dir.path().join("input.mp4");
    // 60 seconds + slow preset so transcode takes long enough to cancel mid-run
    // (modern hardware can encode 10s video in ~50ms, so we need a longer video)
    let duration_secs = 60.0_f32;
    let status = {
        #[cfg(not(feature = "lgpl-macos"))]
        {
            Command::new(&ffmpeg)
                .args([
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    &format!("testsrc=duration={}:size=320x240:rate=30", duration_secs),
                    "-c:v",
                    "libx264",
                    "-pix_fmt",
                    "yuv420p",
                    input_path.to_str().unwrap(),
                ])
                .status()
        }
        #[cfg(feature = "lgpl-macos")]
        {
            Command::new(&ffmpeg)
                .args([
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    &format!("testsrc=duration={}:size=320x240:rate=30", duration_secs),
                    "-c:v",
                    "h264_videotoolbox",
                    "-allow_sw",
                    "1",
                    "-q:v",
                    "25",
                    input_path.to_str().unwrap(),
                ])
                .status()
        }
    }
    .expect("failed to create test video");
    assert!(status.success(), "ffmpeg failed to create test video");

    let options = TranscodeOptions {
        codec: Some(if cfg!(feature = "lgpl-macos") {
            "h264_videotoolbox".to_string()
        } else {
            "libx264".to_string()
        }),
        quality: Some(75),
        max_bitrate: None,
        scale: Some(1.0),
        fps: Some(30),
        remove_audio: Some(true),
        preset: Some("slow".to_string()),
        tune: None,
        output_format: None,
        preview_duration: None,
        duration_secs: None,
    };

    cleanup_transcode_temp();

    let temp = TempFileManager::default();
    let temp_path = temp
        .create("transcode-output.mp4", None)
        .expect("failed to create temp");
    let output_str = temp_path.to_string_lossy().to_string();
    set_transcode_temp(Some(temp_path.clone()));

    let args = build_ffmpeg_command(
        input_path.to_str().unwrap(),
        &output_str,
        &options,
    )
    .expect("build_ffmpeg_command");

    let result_handle = thread::spawn(move || {
        // Small delay to let FFmpeg start, then cancel mid-run
        thread::sleep(StdDuration::from_millis(50));
        crate::ffmpeg::terminate_all_ffmpeg();
    });

    let transcode_result = run_ffmpeg_blocking(
        args,
        None,
        None,
        Some(duration_secs as f64),
        None,
    );

    result_handle.join().unwrap();

    assert!(
        transcode_result.is_err(),
        "expected Aborted, got {:?}",
        transcode_result
    );
    assert!(
        format!("{:?}", transcode_result.err().unwrap()).contains("Aborted"),
        "expected Aborted error"
    );

    cleanup_transcode_temp();

    assert!(
        !temp_path.exists(),
        "temp file should be cleaned up after cancel: {:?}",
        temp_path
    );
}
