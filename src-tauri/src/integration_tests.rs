//! FFmpeg integration test. Requires FFmpeg on the system; run with:
//! `cargo test ffmpeg_transcode_integration -- --ignored`

use crate::ffmpeg::{
    build_ffmpeg_command, run_ffmpeg_blocking, verify_video, TranscodeOptions,
};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

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
