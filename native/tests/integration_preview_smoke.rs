#![cfg(feature = "integration-test-api")]

mod support;

use std::fs;
use std::path::Path;

use support::{
    CodecContract, IntegrationEnv, VideoKind, assert_codec_contract, default_codec, opts_with,
    preview_options, run_preview_and_assert_exists, run_preview_with_estimate_and_assert,
    run_preview_with_meta_codec_override_and_assert_exists, run_transcode_and_verify,
};
use tiny_vid_core::ffmpeg::ffprobe::get_video_metadata_impl;
use tiny_vid_core::ffmpeg::{cleanup_preview_transcode_cache, verify_video};

#[test]
fn preview_generates_single_segment_output() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 2.0, VideoKind::Plain);
    run_preview_and_assert_exists(&input_path, &preview_options(3), None);
}

#[test]
fn preview_generates_multi_segment_output() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 10.0, VideoKind::Plain);
    run_preview_and_assert_exists(&input_path, &preview_options(3), None);
}

#[test]
fn preview_estimate_contract_is_consistent() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 5.0, VideoKind::Plain);

    let result = run_preview_with_estimate_and_assert(&input_path, &preview_options(3), None);
    let estimate = result.estimate.expect("estimate");

    assert!(estimate.best_size > 0);
    assert!(estimate.low_size <= estimate.best_size);
    assert!(estimate.high_size >= estimate.best_size);
    assert!(matches!(
        estimate.confidence,
        tiny_vid_core::ffmpeg::EstimateConfidence::High
            | tiny_vid_core::ffmpeg::EstimateConfidence::Medium
            | tiny_vid_core::ffmpeg::EstimateConfidence::Low
    ));
    assert_eq!(estimate.method, "sampled_bitrate");
    assert!(estimate.sample_count > 0);
    assert!(estimate.sample_seconds_total > 0.0);
}

#[test]
fn preview_estimate_tracks_full_transcode_size_within_reasonable_error() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 12.0, VideoKind::Plain);
    let options = opts_with(|o| {
        o.codec = Some(default_codec());
        o.remove_audio = Some(false);
        o.preset = Some("fast".into());
        o.preview_duration = Some(3);
        o.output_format = Some("mp4".into());
    });
    let output_path = env.path("full-transcode-output.mp4");

    let input_size = fs::metadata(&input_path).expect("input metadata").len();
    let result = run_preview_with_estimate_and_assert(&input_path, &options, None);
    let estimate = result.estimate.expect("estimate");
    run_transcode_and_verify(&input_path, &output_path, &options, None).expect("full transcode");
    let actual_size = fs::metadata(&output_path).expect("output metadata").len();

    assert!(
        actual_size < input_size,
        "transcode should reduce size (input={}, actual={})",
        input_size,
        actual_size
    );

    let actual_size_f64 = actual_size as f64;
    let estimate_f64 = estimate.best_size as f64;
    let absolute_percentage_error =
        ((estimate_f64 - actual_size_f64).abs() / actual_size_f64).abs();

    assert!(
        absolute_percentage_error <= 0.50,
        "estimate APE too high: {:.3} (estimate={}, actual={})",
        absolute_percentage_error,
        estimate.best_size,
        actual_size
    );
}

#[test]
fn preview_estimate_without_audio_is_smaller_than_input_size() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input_multi_audio.mp4", 5.0, VideoKind::MultiAudio(2));
    let options_without_preserve = opts_with(|o| {
        o.remove_audio = Some(true);
        o.preset = Some("ultrafast".into());
        o.preview_duration = Some(3);
        o.codec = Some(default_codec());
    });
    let options_with_preserve = opts_with(|o| {
        o.remove_audio = Some(true);
        o.preset = Some("ultrafast".into());
        o.preview_duration = Some(3);
        o.preserve_additional_audio_streams = Some(true);
        o.audio_stream_count = Some(2);
        o.codec = Some(default_codec());
    });

    let input_size = fs::metadata(&input_path).expect("input metadata").len();
    let estimate_without =
        run_preview_with_estimate_and_assert(&input_path, &options_without_preserve, None)
            .estimate
            .expect("estimate_without")
            .best_size;
    let estimate_with =
        run_preview_with_estimate_and_assert(&input_path, &options_with_preserve, None)
            .estimate
            .expect("estimate_with")
            .best_size;

    assert!(
        estimate_without < input_size && estimate_with < input_size,
        "remove_audio estimates should be below input size (input={}, without={}, with={})",
        input_size,
        estimate_without,
        estimate_with
    );
}

#[test]
fn preview_estimate_increases_with_preserved_additional_audio_streams() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input_multi_audio.mp4", 5.0, VideoKind::MultiAudio(2));

    let options_without_preserve = opts_with(|o| {
        o.remove_audio = Some(false);
        o.preset = Some("ultrafast".into());
        o.preview_duration = Some(3);
        o.codec = Some(default_codec());
    });
    let options_with_preserve = opts_with(|o| {
        o.remove_audio = Some(false);
        o.preset = Some("ultrafast".into());
        o.preview_duration = Some(3);
        o.preserve_additional_audio_streams = Some(true);
        o.audio_stream_count = Some(2);
        o.codec = Some(default_codec());
    });

    let result_without =
        run_preview_with_estimate_and_assert(&input_path, &options_without_preserve, None);
    let result_with =
        run_preview_with_estimate_and_assert(&input_path, &options_with_preserve, None);

    let estimate_without = result_without.estimate.expect("estimate_without").best_size;
    let estimate_with = result_with.estimate.expect("estimate_with").best_size;
    assert!(
        estimate_with > estimate_without,
        "estimate with preserve ({}) should be larger than without ({})",
        estimate_with,
        estimate_without
    );
}

#[test]
fn preview_region_without_estimate_returns_paths() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 10.0, VideoKind::Plain);
    run_preview_and_assert_exists(&input_path, &preview_options(3), Some(2.0));
}

#[test]
fn preview_region_with_estimate_returns_size() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 10.0, VideoKind::Plain);
    let result = run_preview_with_estimate_and_assert(&input_path, &preview_options(3), Some(2.0));
    assert!(result.estimate.expect("estimate").best_size > 0);
}

#[test]
fn preview_output_decodes_successfully() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 5.0, VideoKind::Plain);
    let result = run_preview_and_assert_exists(&input_path, &preview_options(3), None);

    let compressed_path = Path::new(&result.compressed_path);
    let verify_result = verify_video(compressed_path, Some(default_codec().as_str()));
    if let Err(err) = verify_result {
        panic!("compressed preview should decode: {}", err);
    }
}

#[test]
fn preview_original_codec_policy_is_platform_scoped() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 5.0, VideoKind::Plain);
    cleanup_preview_transcode_cache();

    let result = run_preview_with_meta_codec_override_and_assert_exists(
        &input_path,
        &preview_options(3),
        None,
        "hevc",
    );
    let original_file_name = Path::new(&result.original_path)
        .file_name()
        .and_then(|name| name.to_str())
        .expect("original preview file name");

    if cfg!(target_os = "linux") {
        assert!(
            original_file_name.ends_with("preview-original-transcoded.mp4"),
            "linux should force HEVC source metadata down the transcode path"
        );
        let original_meta = get_video_metadata_impl(Path::new(&result.original_path))
            .expect("ffprobe original preview");
        assert_eq!(
            original_meta.codec_name.as_deref(),
            Some("h264"),
            "linux transcode path should produce H.264 original preview"
        );
    } else {
        assert!(
            original_file_name.ends_with("preview-original-0.mp4"),
            "macOS/windows should keep stream-copy path for HEVC metadata override"
        );
    }
}

#[test]
fn preview_cache_reuses_path_for_same_options() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 5.0, VideoKind::Plain);
    let options = preview_options(3);

    let result_a = run_preview_and_assert_exists(&input_path, &options, None);
    let result_b = run_preview_and_assert_exists(&input_path, &options, None);
    assert_eq!(
        result_a.compressed_path, result_b.compressed_path,
        "second run should return cached transcoded output (same path)"
    );
}

#[test]
fn preview_cache_separates_entries_by_options() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 5.0, VideoKind::Plain);
    cleanup_preview_transcode_cache();

    let options_a = opts_with(|o| {
        o.codec = Some(default_codec());
        o.remove_audio = Some(true);
        o.preset = Some("ultrafast".into());
        o.preview_duration = Some(3);
    });
    let options_b = opts_with(|o| {
        o.codec = Some(default_codec());
        o.remove_audio = Some(true);
        o.preset = Some("fast".into());
        o.preview_duration = Some(3);
    });

    let result_a_first = run_preview_and_assert_exists(&input_path, &options_a, None);
    let result_b = run_preview_and_assert_exists(&input_path, &options_b, None);
    let result_a_second = run_preview_and_assert_exists(&input_path, &options_a, None);

    assert_eq!(
        result_a_first.compressed_path, result_a_second.compressed_path,
        "second run with preset A should return cached output path from first preset A run"
    );
    assert_ne!(
        result_a_first.compressed_path, result_b.compressed_path,
        "preset B should produce a different output path than preset A"
    );
}
