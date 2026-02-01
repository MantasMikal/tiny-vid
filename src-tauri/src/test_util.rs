//! Shared test helpers for Tauri command tests.
//!
//! Layout: unit tests live in each module (error, ffmpeg/*); command tests
//! use this module and live in `commands_tests.rs`; the FFmpeg integration
//! test lives in `integration_tests.rs`. Run ignored tests with:
//! `cargo test -- --ignored`.

use crate::{
    cleanup_temp_file, ffmpeg_terminate, get_build_variant, get_file_size, get_video_metadata,
    move_compressed_file,
};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{mock_builder, mock_context, noop_assets, INVOKE_KEY};
use tauri::webview::InvokeRequest;

pub fn create_test_app() -> tauri::App<tauri::test::MockRuntime> {
    mock_builder()
        .invoke_handler(tauri::generate_handler![
            get_file_size,
            get_video_metadata,
            get_build_variant,
            ffmpeg_terminate,
            move_compressed_file,
            cleanup_temp_file,
        ])
        .build(mock_context(noop_assets()))
        .expect("failed to build test app")
}

pub fn invoke_request(cmd: &str, body: InvokeBody) -> InvokeRequest {
    InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: "http://tauri.localhost".parse().unwrap(),
        body,
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    }
}
