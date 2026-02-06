//! Shared helpers for fast lib test modules.

use std::path::PathBuf;

use crate::commands;
use crate::AppState;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{mock_builder, mock_context, noop_assets, INVOKE_KEY};
use tauri::webview::InvokeRequest;

pub fn create_test_app() -> tauri::App<tauri::test::MockRuntime> {
    mock_builder()
        .invoke_handler(tauri::generate_handler![
            commands::get_file_size,
            commands::preview_media_bytes,
            commands::get_video_metadata,
            commands::get_build_variant,
            commands::ffmpeg_terminate,
            commands::move_compressed_file,
            commands::cleanup_temp_file,
        ])
        .build(mock_context(noop_assets()))
        .expect("failed to build test app")
}

/// App with file-association support and optional pre-populated pending-opened-files buffer.
pub fn create_test_app_with_file_assoc(
    pending: Option<Vec<PathBuf>>,
) -> tauri::App<tauri::test::MockRuntime> {
    let state = match pending {
        None => AppState::default(),
        Some(paths) => AppState::with_pending(paths),
    };
    mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_file_size,
            commands::preview_media_bytes,
            commands::get_video_metadata,
            commands::get_build_variant,
            commands::ffmpeg_terminate,
            commands::move_compressed_file,
            commands::cleanup_temp_file,
            commands::get_pending_opened_files,
        ])
        .build(mock_context(noop_assets()))
        .expect("failed to build test app")
}

pub fn invoke_request(cmd: &str, body: InvokeBody) -> InvokeRequest {
    InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: "http://tauri.localhost".parse().expect("valid URL"),
        body,
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    }
}
