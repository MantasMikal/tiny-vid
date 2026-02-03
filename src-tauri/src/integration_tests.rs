//! FFmpeg integration test. Requires FFmpeg on the system; run with:
//! `cargo test ffmpeg_transcode_integration -- --ignored`
//! `cargo test ffmpeg_progress_emission_integration -- --ignored`

use crate::ffmpeg::TranscodeOptions;
use crate::ffmpeg::{
    build_ffmpeg_command, cleanup_transcode_temp, run_ffmpeg_blocking, set_transcode_temp,
    verify_video, TempFileManager,
};
use crate::preview::run_preview_core;
use crate::test_util::{create_test_video, find_ffmpeg_and_set_env, opts_with, preview_options};
use std::fs;
use std::sync::Arc;

use parking_lot::Mutex;
use std::thread;
use std::time::Duration as StdDuration;

fn run_transcode_integration(
    options: TranscodeOptions,
    duration_secs: f32,
    skip_if_encoder_missing: bool,
) {
    let ffmpeg = find_ffmpeg_and_set_env();

    let dir = tempfile::tempdir().unwrap();
    let input_path = dir.path().join("input.mp4");
    let output_path = dir.path().join("output.mp4");

    let status =
        create_test_video(&ffmpeg, &input_path, duration_secs).expect("failed to create test video");
    assert!(status.success(), "ffmpeg failed to create test video");

    let args = build_ffmpeg_command(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        &options,
        None,
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
                (opts_with(|o| {
                    o.remove_audio = Some(true);
                    o.preset = Some("ultrafast".into());
                }), 1.0, false),
                (opts_with(|o| {
                    o.remove_audio = Some(false);
                    o.preset = Some("ultrafast".into());
                }), 1.0, false),
                (opts_with(|o| {
                    o.scale = Some(0.5);
                    o.preset = Some("ultrafast".into());
                }), 1.0, false),
                (opts_with(|o| {
                    o.max_bitrate = Some(1000);
                    o.preset = Some("ultrafast".into());
                }), 1.0, false),
                (opts_with(|o| {
                    o.tune = Some("film".into());
                    o.preset = Some("ultrafast".into());
                }), 1.0, false),
                (opts_with(|o| {
                    o.codec = Some("libx265".into());
                    o.preset = Some("ultrafast".into());
                }), 1.0, false),
                (opts_with(|o| {
                    o.codec = Some("libsvtav1".into());
                    o.preset = Some("ultrafast".into());
                }), 1.0, false),
                (opts_with(|o| {
                    o.codec = Some("libsvtav1".into());
                    o.max_bitrate = Some(1000);
                    o.preset = Some("ultrafast".into());
                }), 1.0, false),
                (opts_with(|o| {
                    o.codec = Some("libsvtav1".into());
                    o.scale = Some(0.5);
                    o.preset = Some("ultrafast".into());
                }), 1.0, false),
                (opts_with(|o| {
                    o.fps = Some(24.0);
                    o.preset = Some("ultrafast".into());
                }), 1.0, false),
            ]
        }
        #[cfg(all(feature = "lgpl-macos", not(target_os = "macos")))]
        {
            vec![] // lgpl-macos is macOS-only; skip on other platforms
        }
        #[cfg(all(feature = "lgpl-macos", target_os = "macos"))]
        {
            vec![
                (opts_with(|o| {
                    o.codec = Some("h264_videotoolbox".into());
                    o.output_format = Some("mp4".into());
                }), 1.0, true),
                (opts_with(|o| {
                    o.codec = Some("h264_videotoolbox".into());
                    o.quality = Some(0);
                    o.output_format = Some("mp4".into());
                }), 1.0, true),
                (opts_with(|o| {
                    o.codec = Some("h264_videotoolbox".into());
                    o.quality = Some(100);
                    o.remove_audio = Some(false);
                    o.output_format = Some("mp4".into());
                }), 1.0, true),
                (opts_with(|o| {
                    o.codec = Some("h264_videotoolbox".into());
                    o.scale = Some(0.5);
                    o.output_format = Some("mp4".into());
                }), 1.0, true),
                (opts_with(|o| {
                    o.codec = Some("h264_videotoolbox".into());
                    o.max_bitrate = Some(1000);
                    o.output_format = Some("mp4".into());
                }), 1.0, true),
                (opts_with(|o| {
                    o.codec = Some("hevc_videotoolbox".into());
                    o.output_format = Some("mp4".into());
                }), 1.0, true),
                (opts_with(|o| {
                    o.codec = Some("hevc_videotoolbox".into());
                    o.quality = Some(50);
                    o.scale = Some(0.5);
                    o.output_format = Some("mp4".into());
                }), 1.0, true),
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
    let ffmpeg = find_ffmpeg_and_set_env();

    let dir = tempfile::tempdir().unwrap();
    let input_path = dir.path().join("input.mp4");
    let output_path = dir.path().join("output.mp4");

    // 2 seconds - long enough for multiple progress updates
    let duration_secs = 2.0_f32;
    let status =
        create_test_video(&ffmpeg, &input_path, duration_secs).expect("failed to create test video");
    assert!(status.success(), "ffmpeg failed to create test video");

    let options = opts_with(|o| {
        o.codec = Some(if cfg!(feature = "lgpl-macos") {
            "h264_videotoolbox".into()
        } else {
            "libx264".into()
        });
        o.remove_audio = Some(true);
        o.preset = Some("ultrafast".into());
    });

    let args = build_ffmpeg_command(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        &options,
        None,
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

    let progress_values = progress_collector.lock().clone();

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
    let ffmpeg = find_ffmpeg_and_set_env();

    let dir = tempfile::tempdir().unwrap();
    let input_path = dir.path().join("input.mp4");
    // 60 seconds + slow preset so transcode takes long enough to cancel mid-run
    // (modern hardware can encode 10s video in ~50ms, so we need a longer video)
    let duration_secs = 60.0_f32;
    let status =
        create_test_video(&ffmpeg, &input_path, duration_secs).expect("failed to create test video");
    assert!(status.success(), "ffmpeg failed to create test video");

    let options = opts_with(|o| {
        o.codec = Some(if cfg!(feature = "lgpl-macos") {
            "h264_videotoolbox".into()
        } else {
            "libx264".into()
        });
        o.remove_audio = Some(true);
        o.preset = Some("slow".into());
    });

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
        None,
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

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_preview_single_segment_integration -- --ignored --test-threads=1"]
fn ffmpeg_preview_single_segment_integration() {
    let ffmpeg = find_ffmpeg_and_set_env();

    let dir = tempfile::tempdir().unwrap();
    let input_path = dir.path().join("input.mp4");
    let status = create_test_video(&ffmpeg, &input_path, 2.0).expect("failed to create test video");
    assert!(status.success(), "ffmpeg failed to create test video");

    let input_size = fs::metadata(&input_path).unwrap().len();
    let result = tauri::async_runtime::block_on(run_preview_core(
        input_path,
        preview_options(3),
        None,
    ))
    .expect("run_preview_core");

    assert!(std::path::Path::new(&result.original_path).exists());
    assert!(std::path::Path::new(&result.compressed_path).exists());
    assert!(result.estimated_size > 0);
    assert!(
        result.estimated_size <= input_size * 2,
        "estimated_size should be reasonable (not >> input): {} > {}",
        result.estimated_size,
        input_size * 2
    );
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_preview_multi_segment_integration -- --ignored --test-threads=1"]
fn ffmpeg_preview_multi_segment_integration() {
    let ffmpeg = find_ffmpeg_and_set_env();

    let dir = tempfile::tempdir().unwrap();
    let input_path = dir.path().join("input.mp4");
    let status = create_test_video(&ffmpeg, &input_path, 10.0).expect("failed to create test video");
    assert!(status.success(), "ffmpeg failed to create test video");

    let input_size = fs::metadata(&input_path).unwrap().len();
    let result = tauri::async_runtime::block_on(run_preview_core(
        input_path,
        preview_options(3),
        None,
    ))
    .expect("run_preview_core");

    assert!(std::path::Path::new(&result.original_path).exists());
    assert!(std::path::Path::new(&result.compressed_path).exists());
    assert!(result.estimated_size > 0);
    assert!(
        result.estimated_size <= input_size * 2,
        "estimated_size should be reasonable: {} > {}",
        result.estimated_size,
        input_size * 2
    );
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_preview_estimation_sanity_integration -- --ignored --test-threads=1"]
fn ffmpeg_preview_estimation_sanity_integration() {
    let ffmpeg = find_ffmpeg_and_set_env();

    let dir = tempfile::tempdir().unwrap();
    let input_path = dir.path().join("input.mp4");
    let status = create_test_video(&ffmpeg, &input_path, 5.0).expect("failed to create test video");
    assert!(status.success(), "ffmpeg failed to create test video");

    let input_size = fs::metadata(&input_path).unwrap().len();
    let result = tauri::async_runtime::block_on(run_preview_core(
        input_path,
        preview_options(3),
        None,
    ))
    .expect("run_preview_core");

    assert!(result.estimated_size > 0, "estimated_size should be positive");
    assert!(
        result.estimated_size <= input_size * 2,
        "estimated_size ({}) should be reasonable (not >> input_size {})",
        result.estimated_size,
        input_size
    );
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_preview_output_valid_integration -- --ignored --test-threads=1"]
fn ffmpeg_preview_output_valid_integration() {
    let ffmpeg = find_ffmpeg_and_set_env();

    let dir = tempfile::tempdir().unwrap();
    let input_path = dir.path().join("input.mp4");
    let status = create_test_video(&ffmpeg, &input_path, 5.0).expect("failed to create test video");
    assert!(status.success(), "ffmpeg failed to create test video");

    let result = tauri::async_runtime::block_on(run_preview_core(
        input_path.clone(),
        preview_options(3),
        None,
    ))
    .expect("run_preview_core");

    let compressed_path = std::path::Path::new(&result.compressed_path);
    assert!(compressed_path.exists());

    let codec = if cfg!(feature = "lgpl-macos") {
        Some("h264_videotoolbox")
    } else {
        Some("libx264")
    };
    let verify_result = verify_video(compressed_path, codec);
    assert!(
        verify_result.is_ok(),
        "compressed preview should decode: {}",
        verify_result.unwrap_err()
    );
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_preview_transcode_cache_integration -- --ignored --test-threads=1"]
fn ffmpeg_preview_transcode_cache_integration() {
    let ffmpeg = find_ffmpeg_and_set_env();

    let dir = tempfile::tempdir().unwrap();
    let input_path = dir.path().join("input.mp4");
    let status = create_test_video(&ffmpeg, &input_path, 5.0).expect("failed to create test video");
    assert!(status.success(), "ffmpeg failed to create test video");

    let opts = preview_options(3);

    let result1 = tauri::async_runtime::block_on(run_preview_core(
        input_path.clone(),
        opts.clone(),
        None,
    ))
    .expect("run_preview_core");

    let result2 = tauri::async_runtime::block_on(run_preview_core(
        input_path,
        opts,
        None,
    ))
    .expect("run_preview_core");

    assert_eq!(
        result1.compressed_path,
        result2.compressed_path,
        "second run should return cached transcoded output (same path)"
    );
    assert_eq!(
        result1.estimated_size,
        result2.estimated_size,
        "cached result should have same estimated_size"
    );
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_preview_transcode_cache_multi_entry_integration -- --ignored --test-threads=1"]
fn ffmpeg_preview_transcode_cache_multi_entry_integration() {
    use crate::ffmpeg::cleanup_preview_transcode_cache;

    let ffmpeg = find_ffmpeg_and_set_env();

    let dir = tempfile::tempdir().unwrap();
    let input_path = dir.path().join("input.mp4");
    let status = create_test_video(&ffmpeg, &input_path, 5.0).expect("failed to create test video");
    assert!(status.success(), "ffmpeg failed to create test video");

    cleanup_preview_transcode_cache();

    let opts_a = opts_with(|o| {
        o.codec = Some(if cfg!(feature = "lgpl-macos") {
            "h264_videotoolbox".into()
        } else {
            "libx264".into()
        });
        o.remove_audio = Some(true);
        o.preset = Some("ultrafast".into());
        o.preview_duration = Some(3);
    });
    let opts_b = opts_with(|o| {
        o.codec = Some(if cfg!(feature = "lgpl-macos") {
            "h264_videotoolbox".into()
        } else {
            "libx264".into()
        });
        o.remove_audio = Some(true);
        o.preset = Some("fast".into());
        o.preview_duration = Some(3);
    });

    let result_a1 = tauri::async_runtime::block_on(run_preview_core(
        input_path.clone(),
        opts_a.clone(),
        None,
    ))
    .expect("run_preview_core");

    let result_b = tauri::async_runtime::block_on(run_preview_core(
        input_path.clone(),
        opts_b.clone(),
        None,
    ))
    .expect("run_preview_core");

    let result_a2 = tauri::async_runtime::block_on(run_preview_core(
        input_path,
        opts_a,
        None,
    ))
    .expect("run_preview_core");

    assert_eq!(
        result_a1.compressed_path,
        result_a2.compressed_path,
        "second run with preset A should return cached transcoded output (same path as first A)"
    );
    assert_eq!(
        result_a1.estimated_size,
        result_a2.estimated_size,
        "cached result should have same estimated_size"
    );
    assert_ne!(
        result_a1.compressed_path,
        result_b.compressed_path,
        "preset B should produce a different output path than preset A"
    );
}

#[test]
#[ignore = "requires FFmpeg on system to detect codecs; run with: cargo test get_build_variant_returns_valid_codecs -- --ignored"]
fn get_build_variant_returns_valid_codecs() {
    let result = crate::codec::get_build_variant();
    assert!(result.is_ok(), "Should detect codecs: {:?}", result.err());
    let variant = result.unwrap();
    assert!(!variant.codecs.is_empty(), "Should have at least one codec");

    #[cfg(feature = "lgpl-macos")]
    assert_eq!(variant.variant, "lgpl-macos");

    #[cfg(not(feature = "lgpl-macos"))]
    assert_eq!(variant.variant, "standalone");

    for codec in &variant.codecs {
        assert!(
            matches!(
                codec.value.as_str(),
                "libx264" | "libx265" | "libsvtav1" | "libvpx-vp9"
                    | "h264_videotoolbox" | "hevc_videotoolbox"
            ),
            "Unexpected codec: {}",
            codec.value
        );
    }
}
