#![cfg(feature = "integration-test-api")]

mod support;

use std::sync::Arc;
use std::thread;
use std::time::Duration as StdDuration;

use parking_lot::Mutex;
use support::{
    assert_codec_contract, default_codec, opts_with, CodecContract, IntegrationEnv, VideoKind,
};
use tiny_vid_tauri_lib::ffmpeg::{
    build_ffmpeg_command, cleanup_transcode_temp, run_ffmpeg_blocking, set_transcode_temp,
    terminate_all_ffmpeg, TempFileManager,
};

#[test]
fn transcode_reports_monotonic_progress() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
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
        input_path.to_string_lossy().as_ref(),
        output_path.to_string_lossy().as_ref(),
        &options,
        None,
        None,
        None,
    )
    .expect("build_ffmpeg_command");

    let progress_values: Arc<Mutex<Vec<f64>>> = Arc::new(Mutex::new(Vec::new()));
    let result = run_ffmpeg_blocking(
        args,
        None,
        None,
        Some(duration_secs),
        None,
        Some(Arc::clone(&progress_values)),
    );

    assert!(result.is_ok(), "run_ffmpeg_blocking failed: {:?}", result.err());

    let progress_values = progress_values.lock().clone();
    assert!(!progress_values.is_empty(), "expected at least one progress value");
    assert!(
        progress_values.last().copied().unwrap_or(0.0) >= 0.98,
        "expected progress to reach ~1.0, got {:?}",
        progress_values.last()
    );
    for window in progress_values.windows(2) {
        assert!(window[1] >= window[0], "progress should increase: {:?}", progress_values);
    }
}

#[test]
fn transcode_cancel_cleans_up_temp_output() {
    assert_codec_contract(CodecContract::IntegrationSmoke);
    let env = IntegrationEnv::new();
    let input_path = env.with_test_video("input.mp4", 60.0, VideoKind::Plain);
    let duration_secs = 60.0_f64;

    let options = opts_with(|o| {
        o.codec = Some(default_codec());
        o.remove_audio = Some(true);
        o.preset = Some("slow".into());
    });

    cleanup_transcode_temp();

    let temp_manager = TempFileManager::default();
    let temp_path = temp_manager
        .create("transcode-output.mp4", None)
        .expect("failed to create temp output");
    set_transcode_temp(Some(temp_path.clone()));

    let args = build_ffmpeg_command(
        input_path.to_string_lossy().as_ref(),
        temp_path.to_string_lossy().as_ref(),
        &options,
        None,
        None,
        None,
    )
    .expect("build_ffmpeg_command");

    let terminate_handle = thread::spawn(move || {
        thread::sleep(StdDuration::from_millis(50));
        terminate_all_ffmpeg();
    });

    let transcode_result = run_ffmpeg_blocking(args, None, None, Some(duration_secs), None, None);
    terminate_handle.join().expect("join");

    assert!(transcode_result.is_err(), "expected Aborted, got {:?}", transcode_result);
    assert!(
        format!("{:?}", transcode_result.expect_err("expected error")).contains("Aborted"),
        "expected Aborted error"
    );

    cleanup_transcode_temp();
    assert!(
        !temp_path.exists(),
        "temp file should be cleaned up after cancel: {:?}",
        temp_path
    );
}
