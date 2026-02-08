#![cfg(feature = "integration-test-api")]

mod support;

use std::fs;

use support::{
    CodecContract, IntegrationEnv, VideoKind, assert_codec_contract, metadata, opts_with,
    run_transcode_and_verify,
};
use tiny_vid_core::ffmpeg::{TranscodeOptions, build_ffmpeg_command, run_ffmpeg_blocking};

fn run_transcode_case(options: TranscodeOptions, duration_secs: f32) {
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", duration_secs, VideoKind::Plain);
    let ext = if options
        .output_format
        .as_deref()
        .is_some_and(|f| f.eq_ignore_ascii_case("webm"))
    {
        "webm"
    } else if options
        .output_format
        .as_deref()
        .is_some_and(|f| f.eq_ignore_ascii_case("mkv"))
    {
        "mkv"
    } else {
        "mp4"
    };
    let output_path = env.path(&format!("output.{}", ext));
    run_transcode_and_verify(&input_path, &output_path, &options, None).expect("transcode failed");
}

#[test]
fn transcode_codec_matrix_satisfies_contract() {
    assert_codec_contract(CodecContract::IntegrationContract);
    let option_sets: Vec<TranscodeOptions> = {
        #[cfg(not(feature = "lgpl"))]
        {
            vec![
                opts_with(|o| {
                    o.codec = Some("libx264".into());
                    o.preset = Some("ultrafast".into());
                }),
                opts_with(|o| {
                    o.codec = Some("libx264".into());
                    o.tune = Some("film".into());
                    o.preset = Some("ultrafast".into());
                }),
                opts_with(|o| {
                    o.codec = Some("libx265".into());
                    o.preset = Some("ultrafast".into());
                }),
                opts_with(|o| {
                    o.codec = Some("libsvtav1".into());
                    o.preset = Some("ultrafast".into());
                }),
                opts_with(|o| {
                    o.codec = Some("libsvtav1".into());
                    o.max_bitrate = Some(1000);
                    o.preset = Some("ultrafast".into());
                }),
                opts_with(|o| {
                    o.codec = Some("libsvtav1".into());
                    o.scale = Some(0.5);
                    o.preset = Some("ultrafast".into());
                }),
                opts_with(|o| {
                    o.codec = Some("libvpx-vp9".into());
                    o.preset = Some("ultrafast".into());
                    o.output_format = Some("mkv".into());
                }),
            ]
        }
        #[cfg(feature = "lgpl")]
        {
            vec![
                opts_with(|o| {
                    o.codec = Some("h264_videotoolbox".into());
                    o.output_format = Some("mp4".into());
                }),
                opts_with(|o| {
                    o.codec = Some("h264_videotoolbox".into());
                    o.quality = Some(0);
                    o.output_format = Some("mp4".into());
                }),
                opts_with(|o| {
                    o.codec = Some("h264_videotoolbox".into());
                    o.quality = Some(100);
                    o.remove_audio = Some(false);
                    o.output_format = Some("mp4".into());
                }),
                opts_with(|o| {
                    o.codec = Some("h264_videotoolbox".into());
                    o.scale = Some(0.5);
                    o.output_format = Some("mp4".into());
                }),
                opts_with(|o| {
                    o.codec = Some("h264_videotoolbox".into());
                    o.max_bitrate = Some(1000);
                    o.output_format = Some("mp4".into());
                }),
                opts_with(|o| {
                    o.codec = Some("hevc_videotoolbox".into());
                    o.output_format = Some("mp4".into());
                }),
                opts_with(|o| {
                    o.codec = Some("hevc_videotoolbox".into());
                    o.quality = Some(50);
                    o.scale = Some(0.5);
                    o.output_format = Some("mp4".into());
                }),
                opts_with(|o| {
                    o.codec = Some("libsvtav1".into());
                    o.preset = Some("ultrafast".into());
                    o.output_format = Some("mp4".into());
                }),
                opts_with(|o| {
                    o.codec = Some("libvpx-vp9".into());
                    o.preset = Some("ultrafast".into());
                    o.output_format = Some("mkv".into());
                }),
            ]
        }
    };

    for options in option_sets {
        run_transcode_case(options, 1.0);
    }
}

#[test]
fn transcode_preserves_subtitles_webm() {
    assert_codec_contract(CodecContract::IntegrationContract);
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
        input_path.to_string_lossy().as_ref(),
        output_path.to_string_lossy().as_ref(),
        &options,
        None,
        None,
        None,
    )
    .expect("build_ffmpeg_command");

    let result = run_ffmpeg_blocking(args, None, None, None);
    assert!(
        result.is_ok(),
        "run_ffmpeg_blocking failed: {:?}",
        result.err()
    );
    assert!(output_path.exists());
    assert!(fs::metadata(&output_path).expect("metadata").len() > 0);

    let output_meta = metadata(&output_path);
    assert!(
        output_meta.subtitle_stream_count >= 1,
        "output should preserve subtitle stream, got {}",
        output_meta.subtitle_stream_count
    );
    // WebM decode verification is intentionally not asserted due known Opus parser warnings.
}
