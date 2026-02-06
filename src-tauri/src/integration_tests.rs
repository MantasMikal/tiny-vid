//! FFmpeg integration test. Requires FFmpeg on the system; run with:
//! `cargo test ffmpeg_transcode_integration -- --ignored`
//! `cargo test ffmpeg_progress_emission_integration -- --ignored`

use crate::ffmpeg::TranscodeOptions;
use crate::ffmpeg::ffprobe::get_video_metadata_impl;
use crate::ffmpeg::{
    build_ffmpeg_command, cleanup_transcode_temp, run_ffmpeg_blocking, set_transcode_temp,
    verify_video, TempFileManager,
};
use crate::test_util::{
    default_codec, opts_with, preview_options, run_preview_and_assert_exists,
    run_preview_with_estimate_and_assert, run_transcode_and_verify,
    IntegrationEnv, VideoKind,
};
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
    let env = IntegrationEnv::new();
    env.with_test_video("input.mp4", duration_secs, VideoKind::Plain);
    let input_path = env.path("input.mp4");
    let output_path = env.path("output.mp4");

    run_transcode_and_verify(
        &input_path,
        &output_path,
        &options,
        None,
        skip_if_encoder_missing,
    )
    .expect("transcode failed");
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_transcode_integration -- --ignored"]
fn ffmpeg_transcode_integration() {
    let option_sets: Vec<(TranscodeOptions, f32, bool)> = {
        #[cfg(not(feature = "lgpl"))]
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
        #[cfg(all(feature = "lgpl", not(target_os = "macos")))]
        {
            vec![] // lgpl build is macOS-only for now; skip on other platforms
        }
        #[cfg(all(feature = "lgpl", target_os = "macos"))]
        {
            let c = default_codec();
            vec![
                (opts_with(|o| {
                    o.codec = Some(c.clone());
                    o.output_format = Some("mp4".into());
                }), 1.0, true),
                (opts_with(|o| {
                    o.codec = Some(c.clone());
                    o.quality = Some(0);
                    o.output_format = Some("mp4".into());
                }), 1.0, true),
                (opts_with(|o| {
                    o.codec = Some(c.clone());
                    o.quality = Some(100);
                    o.remove_audio = Some(false);
                    o.output_format = Some("mp4".into());
                }), 1.0, true),
                (opts_with(|o| {
                    o.codec = Some(c.clone());
                    o.scale = Some(0.5);
                    o.output_format = Some("mp4".into());
                }), 1.0, true),
                (opts_with(|o| {
                    o.codec = Some(c.clone());
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
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_transcode_preserve_additional_audio_streams_integration -- --ignored"]
fn ffmpeg_transcode_preserve_additional_audio_streams_integration() {
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input_multi_audio.mp4", 2.0, VideoKind::MultiAudio(2));
    let output_path = env.path("output.mp4");

    let input_meta = get_video_metadata_impl(&input_path).expect("get_video_metadata_impl input");
    assert_eq!(
        input_meta.audio_stream_count, 2,
        "input should have 2 audio streams"
    );

    let options = opts_with(|o| {
        o.remove_audio = Some(false);
        o.preset = Some("ultrafast".into());
        o.preserve_additional_audio_streams = Some(true);
        o.audio_stream_count = Some(2);
        o.codec = Some(default_codec());
    });

    run_transcode_and_verify(&input_path, &output_path, &options, None, false)
        .expect("transcode failed");

    let output_meta =
        get_video_metadata_impl(&output_path).expect("get_video_metadata_impl output");
    assert_eq!(
        output_meta.audio_stream_count, 2,
        "output should preserve 2 audio streams, got {}",
        output_meta.audio_stream_count
    );
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_transcode_preserve_subtitles_mp4_integration -- --ignored"]
fn ffmpeg_transcode_preserve_subtitles_mp4_integration() {
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input_with_subs.mp4", 2.0, VideoKind::Subtitles);
    let output_path = env.path("output.mp4");

    let input_meta = get_video_metadata_impl(&input_path).expect("get_video_metadata_impl input");
    assert_eq!(
        input_meta.subtitle_stream_count, 1,
        "input should have 1 subtitle stream"
    );

    let options = opts_with(|o| {
        o.remove_audio = Some(false);
        o.preset = Some("ultrafast".into());
        o.preserve_subtitles = Some(true);
        o.subtitle_stream_count = Some(1);
        o.codec = Some(default_codec());
    });

    run_transcode_and_verify(&input_path, &output_path, &options, None, false)
        .expect("transcode failed");

    let output_meta =
        get_video_metadata_impl(&output_path).expect("get_video_metadata_impl output");
    assert!(
        output_meta.subtitle_stream_count >= 1,
        "output should preserve subtitle stream, got {}",
        output_meta.subtitle_stream_count
    );
}

#[test]
#[cfg(not(feature = "lgpl"))] // WebM requires libvpx-vp9, not in LGPL build
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_transcode_preserve_subtitles_webm_integration -- --ignored"]
fn ffmpeg_transcode_preserve_subtitles_webm_integration() {
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input_with_subs.mp4", 2.0, VideoKind::Subtitles);
    let output_path = env.path("output.webm");

    let options = opts_with(|o| {
        o.remove_audio = Some(false);
        o.preset = Some("ultrafast".into());
        o.preserve_subtitles = Some(true);
        o.subtitle_stream_count = Some(1);
        o.output_format = Some("webm".into());
        o.codec = Some("libvpx-vp9".into());
    });

    let args = build_ffmpeg_command(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        &options,
        None,
        None,
        None,
    )
    .expect("build_ffmpeg_command");

    let result = run_ffmpeg_blocking(args, None, None, None, None, None);
    if let Err(ref e) = result {
        let stderr = format!("{}", e);
        if stderr.to_lowercase().contains("unknown encoder")
            || stderr.to_lowercase().contains("encoder not found")
        {
            return;
        }
    }
    assert!(
        result.is_ok(),
        "run_ffmpeg_blocking failed: {:?}",
        result.err()
    );
    assert!(output_path.exists());
    assert!(fs::metadata(&output_path).unwrap().len() > 0);

    let output_meta =
        get_video_metadata_impl(&output_path).expect("get_video_metadata_impl output");
    assert!(
        output_meta.subtitle_stream_count >= 1,
        "output should preserve subtitle stream, got {}",
        output_meta.subtitle_stream_count
    );
    // Note: verify_video is not called for WebM - it fails with Opus packet header parsing errors.
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_transcode_preserve_subtitles_mkv_integration -- --ignored"]
fn ffmpeg_transcode_preserve_subtitles_mkv_integration() {
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input_with_subs.mp4", 2.0, VideoKind::Subtitles);
    let output_path = env.path("output.mkv");

    let options = opts_with(|o| {
        o.remove_audio = Some(false);
        o.preset = Some("ultrafast".into());
        o.preserve_subtitles = Some(true);
        o.subtitle_stream_count = Some(1);
        o.output_format = Some("mkv".into());
        o.codec = Some(default_codec());
    });

    run_transcode_and_verify(&input_path, &output_path, &options, None, false)
        .expect("transcode failed");

    let output_meta =
        get_video_metadata_impl(&output_path).expect("get_video_metadata_impl output");
    assert!(
        output_meta.subtitle_stream_count >= 1,
        "output should preserve subtitle stream, got {}",
        output_meta.subtitle_stream_count
    );
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_transcode_preserve_subtitles_no_subs_optional_map_integration -- --ignored"]
fn ffmpeg_transcode_preserve_subtitles_no_subs_optional_map_integration() {
    let env = IntegrationEnv::new();
    let input_path =
        env.with_test_video("input.mp4", 2.0, VideoKind::MultiAudio(1));
    let output_path = env.path("output.mp4");

    let options = opts_with(|o| {
        o.remove_audio = Some(false);
        o.preset = Some("ultrafast".into());
        o.preserve_subtitles = Some(true);
        o.subtitle_stream_count = Some(1);
        o.codec = Some(default_codec());
    });

    run_transcode_and_verify(&input_path, &output_path, &options, None, false)
        .expect("transcode with preserve_subtitles but no subs in input should succeed (0:s? optional map)");
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_transcode_preserve_subtitles_no_audio_integration -- --ignored"]
fn ffmpeg_transcode_preserve_subtitles_no_audio_integration() {
    let env = IntegrationEnv::new();
    let input_path =
        env.with_test_video("input_with_subs_no_audio.mp4", 2.0, VideoKind::SubtitlesNoAudio);
    let output_path = env.path("output.mp4");

    let input_meta = get_video_metadata_impl(&input_path).expect("get_video_metadata_impl input");
    assert_eq!(
        input_meta.audio_stream_count, 0,
        "input should have 0 audio streams"
    );
    assert_eq!(
        input_meta.subtitle_stream_count, 1,
        "input should have 1 subtitle stream"
    );

    let options = opts_with(|o| {
        o.remove_audio = Some(false);
        o.preset = Some("ultrafast".into());
        o.preserve_subtitles = Some(true);
        o.subtitle_stream_count = Some(1);
        o.codec = Some(default_codec());
    });

    run_transcode_and_verify(&input_path, &output_path, &options, None, false)
        .expect("transcode with subtitles and no audio should succeed");

    let output_meta =
        get_video_metadata_impl(&output_path).expect("get_video_metadata_impl output");
    assert_eq!(
        output_meta.audio_stream_count, 0,
        "output should have 0 audio streams, got {}",
        output_meta.audio_stream_count
    );
    assert!(
        output_meta.subtitle_stream_count >= 1,
        "output should preserve subtitle stream, got {}",
        output_meta.subtitle_stream_count
    );
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_progress_emission_integration -- --ignored"]
fn ffmpeg_progress_emission_integration() {
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 2.0, VideoKind::Plain);
    let output_path = env.path("output.mp4");
    let duration_secs = 2.0_f64;

    let options = opts_with(|o| {
        o.codec = Some(default_codec());
        o.remove_audio = Some(true);
        o.preset = Some("ultrafast".into());
    });

    let args = build_ffmpeg_command(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        &options,
        None,
        None,
        None,
    )
    .expect("build_ffmpeg_command");

    let progress_collector: Arc<Mutex<Vec<f64>>> = Arc::new(Mutex::new(Vec::new()));
    let result = run_ffmpeg_blocking(
        args,
        None,
        None,
        Some(duration_secs),
        None,
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
    let env = IntegrationEnv::new();
    // 60 seconds + slow preset so transcode takes long enough to cancel mid-run
    let input_path = env.with_test_video("input.mp4", 60.0, VideoKind::Plain);
    let duration_secs = 60.0_f64;

    let options = opts_with(|o| {
        o.codec = Some(default_codec());
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
        None,
        None,
    )
    .expect("build_ffmpeg_command");

    let result_handle = thread::spawn(move || {
        thread::sleep(StdDuration::from_millis(50));
        crate::ffmpeg::terminate_all_ffmpeg();
    });

    let transcode_result = run_ffmpeg_blocking(args, None, None, Some(duration_secs), None, None);

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
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 2.0, VideoKind::Plain);
    run_preview_and_assert_exists(&input_path, &preview_options(3), None);
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_preview_multi_segment_integration -- --ignored --test-threads=1"]
fn ffmpeg_preview_multi_segment_integration() {
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 10.0, VideoKind::Plain);
    run_preview_and_assert_exists(&input_path, &preview_options(3), None);
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_preview_estimation_sanity_integration -- --ignored --test-threads=1"]
fn ffmpeg_preview_estimation_sanity_integration() {
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 5.0, VideoKind::Plain);
    let input_size = fs::metadata(&input_path).unwrap().len();

    let result = run_preview_with_estimate_and_assert(&input_path, &preview_options(3), None);
    let estimated_size = result.estimated_size.unwrap();
    assert!(estimated_size > 0, "estimated_size should be positive");
    assert!(
        estimated_size <= input_size * 2,
        "estimated_size ({}) should be reasonable (not >> input_size {})",
        estimated_size,
        input_size
    );
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_preview_estimate_preserve_additional_audio_streams_integration -- --ignored --test-threads=1"]
fn ffmpeg_preview_estimate_preserve_additional_audio_streams_integration() {
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input_multi_audio.mp4", 5.0, VideoKind::MultiAudio(2));

    let opts_without_preserve = opts_with(|o| {
        o.remove_audio = Some(false);
        o.preset = Some("ultrafast".into());
        o.preview_duration = Some(3);
        o.codec = Some(default_codec());
    });
    let opts_with_preserve = opts_with(|o| {
        o.remove_audio = Some(false);
        o.preset = Some("ultrafast".into());
        o.preview_duration = Some(3);
        o.preserve_additional_audio_streams = Some(true);
        o.audio_stream_count = Some(2);
        o.codec = Some(default_codec());
    });

    let result_without =
        run_preview_with_estimate_and_assert(&input_path, &opts_without_preserve, None);
    let result_with = run_preview_with_estimate_and_assert(&input_path, &opts_with_preserve, None);

    let estimate_without = result_without.estimated_size.unwrap();
    let estimate_with = result_with.estimated_size.unwrap();
    assert!(
        estimate_with > estimate_without,
        "estimate with preserve ({}) should be larger than without ({})",
        estimate_with,
        estimate_without
    );
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_preview_region_no_estimate_integration -- --ignored --test-threads=1"]
fn ffmpeg_preview_region_no_estimate_integration() {
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 10.0, VideoKind::Plain);
    run_preview_and_assert_exists(&input_path, &preview_options(3), Some(2.0));
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_preview_region_with_estimate_integration -- --ignored --test-threads=1"]
fn ffmpeg_preview_region_with_estimate_integration() {
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 10.0, VideoKind::Plain);
    let result =
        run_preview_with_estimate_and_assert(&input_path, &preview_options(3), Some(2.0));
    assert!(result.estimated_size.unwrap() > 0);
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_preview_output_valid_integration -- --ignored --test-threads=1"]
fn ffmpeg_preview_output_valid_integration() {
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 5.0, VideoKind::Plain);
    let result = run_preview_and_assert_exists(&input_path, &preview_options(3), None);

    let compressed_path = std::path::Path::new(&result.compressed_path);
    let verify_result =
        verify_video(compressed_path, Some(default_codec().as_str()));
    assert!(
        verify_result.is_ok(),
        "compressed preview should decode: {}",
        verify_result.unwrap_err()
    );
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_preview_transcode_cache_integration -- --ignored --test-threads=1"]
fn ffmpeg_preview_transcode_cache_integration() {
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 5.0, VideoKind::Plain);
    let opts = preview_options(3);

    let result1 = run_preview_and_assert_exists(&input_path, &opts, None);
    let result2 = run_preview_and_assert_exists(&input_path, &opts, None);
    assert_eq!(
        result1.compressed_path,
        result2.compressed_path,
        "second run should return cached transcoded output (same path)"
    );
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test ffmpeg_preview_transcode_cache_multi_entry_integration -- --ignored --test-threads=1"]
fn ffmpeg_preview_transcode_cache_multi_entry_integration() {
    use crate::ffmpeg::cleanup_preview_transcode_cache;

    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 5.0, VideoKind::Plain);

    cleanup_preview_transcode_cache();

    let opts_a = opts_with(|o| {
        o.codec = Some(default_codec());
        o.remove_audio = Some(true);
        o.preset = Some("ultrafast".into());
        o.preview_duration = Some(3);
    });
    let opts_b = opts_with(|o| {
        o.codec = Some(default_codec());
        o.remove_audio = Some(true);
        o.preset = Some("fast".into());
        o.preview_duration = Some(3);
    });

    let result_a1 = run_preview_and_assert_exists(&input_path, &opts_a, None);
    let result_b = run_preview_and_assert_exists(&input_path, &opts_b, None);
    let result_a2 = run_preview_and_assert_exists(&input_path, &opts_a, None);

    assert_eq!(
        result_a1.compressed_path,
        result_a2.compressed_path,
        "second run with preset A should return cached transcoded output (same path as first A)"
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
    let available = crate::ffmpeg::discovery::get_available_codecs().expect("get_available_codecs");
    let result = crate::codec::get_build_variant(available);
    assert!(result.is_ok(), "Should detect codecs: {:?}", result.err());
    let variant = result.unwrap();
    assert!(!variant.codecs.is_empty(), "Should have at least one codec");

    #[cfg(feature = "lgpl")]
    assert_eq!(variant.variant, "lgpl");

    #[cfg(not(feature = "lgpl"))]
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
