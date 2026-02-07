#![cfg(feature = "integration-test-api")]

mod support;

use support::{CodecContract, IntegrationEnv, VideoKind, assert_codec_contract};

#[test]
fn command_get_video_metadata_returns_metadata_for_video() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 2.0, VideoKind::Plain);

    let metadata =
        tiny_vid_tauri_lib::test_support::get_video_metadata_via_command_for_test(input_path)
            .expect("get_video_metadata_via_command_for_test");

    assert!(
        metadata.duration > 1.0 && metadata.duration < 3.0,
        "duration={}",
        metadata.duration
    );
    assert_eq!(metadata.width, 320);
    assert_eq!(metadata.height, 240);
    assert!(metadata.size > 0);
}

#[test]
fn command_get_video_metadata_returns_audio_stream_count_for_multi_audio_input() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("multi_audio.mp4", 2.0, VideoKind::MultiAudio(2));

    let metadata =
        tiny_vid_tauri_lib::test_support::get_video_metadata_via_command_for_test(input_path)
            .expect("get_video_metadata_via_command_for_test");

    assert!(
        metadata.duration > 1.0 && metadata.duration < 3.0,
        "duration={}",
        metadata.duration
    );
    assert_eq!(metadata.width, 320);
    assert_eq!(metadata.height, 240);
    assert!(metadata.size > 0);
    assert_eq!(
        metadata.audio_stream_count, 2,
        "audioStreamCount should be 2 for multi-audio file"
    );
}

#[test]
fn command_get_build_variant_returns_known_codecs() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let variant = tiny_vid_tauri_lib::test_support::get_build_variant_via_command_for_test()
        .expect("get_build_variant_via_command_for_test");
    assert!(!variant.codecs.is_empty(), "codecs should not be empty");

    #[cfg(feature = "lgpl")]
    assert_eq!(variant.variant, "lgpl");

    #[cfg(not(feature = "lgpl"))]
    assert_eq!(variant.variant, "standalone");

    for codec in &variant.codecs {
        assert!(
            matches!(
                codec.value.as_str(),
                "libx264"
                    | "libx265"
                    | "libsvtav1"
                    | "libvpx-vp9"
                    | "h264_videotoolbox"
                    | "hevc_videotoolbox"
            ),
            "Unexpected codec: {}",
            codec.value
        );
    }
}
