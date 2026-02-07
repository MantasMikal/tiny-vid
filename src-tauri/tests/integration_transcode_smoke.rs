#![cfg(feature = "integration-test-api")]

mod support;

use support::{
    CodecContract, IntegrationEnv, VideoKind, assert_codec_contract, default_codec, metadata,
    opts_with, run_transcode_and_verify,
};
use tiny_vid_tauri_lib::ffmpeg::TranscodeOptions;

fn run_transcode_case(options: TranscodeOptions, duration_secs: f32) {
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", duration_secs, VideoKind::Plain);
    let output_path = env.path("output.mp4");
    run_transcode_and_verify(&input_path, &output_path, &options, None).expect("transcode failed");
}

#[test]
fn transcode_runs_default_codec_option_matrix() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let codec = default_codec();
    let option_sets = vec![
        opts_with(|o| {
            o.codec = Some(codec.clone());
            o.remove_audio = Some(true);
            o.preset = Some("ultrafast".into());
        }),
        opts_with(|o| {
            o.codec = Some(codec.clone());
            o.remove_audio = Some(false);
            o.preset = Some("ultrafast".into());
        }),
        opts_with(|o| {
            o.codec = Some(codec.clone());
            o.scale = Some(0.5);
            o.preset = Some("ultrafast".into());
        }),
        opts_with(|o| {
            o.codec = Some(codec.clone());
            o.max_bitrate = Some(1000);
            o.preset = Some("ultrafast".into());
        }),
        opts_with(|o| {
            o.codec = Some(codec.clone());
            o.fps = Some(24.0);
            o.preset = Some("ultrafast".into());
        }),
    ];

    for options in option_sets {
        run_transcode_case(options, 1.0);
    }
}

#[test]
fn transcode_preserves_additional_audio_streams() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input_multi_audio.mp4", 2.0, VideoKind::MultiAudio(2));
    let output_path = env.path("output.mp4");

    let input_meta = metadata(&input_path);
    assert_eq!(input_meta.audio_stream_count, 2);

    let options = opts_with(|o| {
        o.remove_audio = Some(false);
        o.preset = Some("ultrafast".into());
        o.preserve_additional_audio_streams = Some(true);
        o.audio_stream_count = Some(2);
        o.codec = Some(default_codec());
    });

    run_transcode_and_verify(&input_path, &output_path, &options, None).expect("transcode failed");

    let output_meta = metadata(&output_path);
    assert_eq!(
        output_meta.audio_stream_count, 2,
        "output should preserve 2 audio streams, got {}",
        output_meta.audio_stream_count
    );
}

#[test]
fn transcode_preserves_subtitles_mp4() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input_with_subs.mp4", 2.0, VideoKind::Subtitles);
    let output_path = env.path("output.mp4");

    let input_meta = metadata(&input_path);
    assert_eq!(input_meta.subtitle_stream_count, 1);

    let options = opts_with(|o| {
        o.remove_audio = Some(false);
        o.preset = Some("ultrafast".into());
        o.preserve_subtitles = Some(true);
        o.subtitle_stream_count = Some(1);
        o.codec = Some(default_codec());
    });

    run_transcode_and_verify(&input_path, &output_path, &options, None).expect("transcode failed");

    let output_meta = metadata(&output_path);
    assert!(
        output_meta.subtitle_stream_count >= 1,
        "output should preserve subtitle stream, got {}",
        output_meta.subtitle_stream_count
    );
}

#[test]
fn transcode_preserves_subtitles_mkv() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
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

    run_transcode_and_verify(&input_path, &output_path, &options, None).expect("transcode failed");

    let output_meta = metadata(&output_path);
    assert!(
        output_meta.subtitle_stream_count >= 1,
        "output should preserve subtitle stream, got {}",
        output_meta.subtitle_stream_count
    );
}

#[test]
fn transcode_preserve_subtitles_optional_map_succeeds_without_input_subtitles() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 2.0, VideoKind::MultiAudio(1));
    let output_path = env.path("output.mp4");

    let options = opts_with(|o| {
        o.remove_audio = Some(false);
        o.preset = Some("ultrafast".into());
        o.preserve_subtitles = Some(true);
        o.subtitle_stream_count = Some(1);
        o.codec = Some(default_codec());
    });

    run_transcode_and_verify(&input_path, &output_path, &options, None)
        .expect("transcode should succeed with optional subtitle mapping");
}

#[test]
fn transcode_preserves_subtitles_when_input_has_no_audio() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video(
        "input_with_subs_no_audio.mp4",
        2.0,
        VideoKind::SubtitlesNoAudio,
    );
    let output_path = env.path("output.mp4");

    let input_meta = metadata(&input_path);
    assert_eq!(input_meta.audio_stream_count, 0);
    assert_eq!(input_meta.subtitle_stream_count, 1);

    let options = opts_with(|o| {
        o.remove_audio = Some(false);
        o.preset = Some("ultrafast".into());
        o.preserve_subtitles = Some(true);
        o.subtitle_stream_count = Some(1);
        o.codec = Some(default_codec());
    });

    run_transcode_and_verify(&input_path, &output_path, &options, None)
        .expect("transcode with subtitles and no audio should succeed");

    let output_meta = metadata(&output_path);
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
