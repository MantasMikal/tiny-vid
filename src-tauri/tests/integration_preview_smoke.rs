#![cfg(feature = "integration-test-api")]

mod support;

use std::fs;
use std::path::Path;

use support::{
    assert_codec_contract, default_codec, opts_with, preview_options, run_preview_and_assert_exists,
    run_preview_with_estimate_and_assert,
    run_preview_with_meta_codec_and_audio_override_and_assert_exists,
    run_preview_with_meta_codec_override_and_assert_exists, CodecContract, IntegrationEnv,
    VideoKind,
};
use tiny_vid_tauri_lib::ffmpeg::ffprobe::get_video_metadata_impl;
use tiny_vid_tauri_lib::ffmpeg::{cleanup_preview_transcode_cache, verify_video};

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
fn preview_estimate_is_positive_and_reasonable() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 5.0, VideoKind::Plain);
    let input_size = fs::metadata(&input_path).expect("input metadata").len();

    let result = run_preview_with_estimate_and_assert(&input_path, &preview_options(3), None);
    let estimated_size = result.estimated_size.expect("estimated_size");
    assert!(estimated_size > 0);
    assert!(
        estimated_size <= input_size * 2,
        "estimated_size ({}) should be reasonable (input_size {})",
        estimated_size,
        input_size
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
    let result_with = run_preview_with_estimate_and_assert(&input_path, &options_with_preserve, None);

    let estimate_without = result_without.estimated_size.expect("estimate_without");
    let estimate_with = result_with.estimated_size.expect("estimate_with");
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
    assert!(result.estimated_size.expect("estimated_size") > 0);
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
        "vp9",
    );
    let original_file_name = Path::new(&result.original_path)
        .file_name()
        .and_then(|name| name.to_str())
        .expect("original preview file name");

    if cfg!(target_os = "linux") {
        assert!(
            original_file_name.ends_with("preview-original-transcoded.mp4"),
            "linux should force non-AVC source metadata down the transcode path"
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
            "macOS/windows should keep stream-copy path for vp9 metadata override"
        );
    }
}

#[test]
fn preview_original_audio_codec_policy_is_platform_scoped() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input_multi_audio.mp4", 5.0, VideoKind::MultiAudio(2));
    cleanup_preview_transcode_cache();

    let result = run_preview_with_meta_codec_and_audio_override_and_assert_exists(
        &input_path,
        &preview_options(3),
        None,
        "h264",
        "pcm_s16le",
        1,
    );
    let original_file_name = Path::new(&result.original_path)
        .file_name()
        .and_then(|name| name.to_str())
        .expect("original preview file name");

    if cfg!(target_os = "linux") {
        assert!(
            original_file_name.ends_with("preview-original-transcoded.mp4"),
            "linux should force unsupported audio metadata down the transcode path"
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
            "macOS/windows should keep stream-copy path for audio codec override"
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
