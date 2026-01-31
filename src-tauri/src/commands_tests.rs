//! Tauri IPC command tests. Uses test_util for app and invoke helpers.

use crate::test_util::{create_test_app, invoke_request};
use std::fs;
use tauri::ipc::InvokeBody;

#[test]
fn get_file_size_returns_size() {
    let app = create_test_app();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("failed to create window");

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("testfile");
    fs::write(&path, b"hello").unwrap();

    let body = InvokeBody::from(serde_json::json!({ "path": path.to_string_lossy() }));
    let res = tauri::test::get_ipc_response(&window, invoke_request("get_file_size", body));
    assert!(res.is_ok(), "get_file_size failed: {:?}", res.err());
    let body = res.unwrap();
    let size: u64 = body.deserialize().unwrap();
    assert_eq!(size, 5);
}

#[test]
fn get_file_size_nonexistent_returns_error() {
    let app = create_test_app();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("failed to create window");

    let body = InvokeBody::from(serde_json::json!({
        "path": "/nonexistent/path/that/does/not/exist"
    }));
    let res = tauri::test::get_ipc_response(&window, invoke_request("get_file_size", body));
    assert!(res.is_err());
}

#[test]
fn ffmpeg_terminate_does_not_panic() {
    let app = create_test_app();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("failed to create window");

    let body = InvokeBody::default();
    let _ = tauri::test::get_ipc_response(&window, invoke_request("ffmpeg_terminate", body));
}

#[test]
fn move_compressed_file_renames() {
    let app = create_test_app();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("failed to create window");

    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.mp4");
    let dest = dir.path().join("dest.mp4");
    fs::write(&source, b"video data").unwrap();

    let body = InvokeBody::from(serde_json::json!({
        "source": source.to_string_lossy(),
        "dest": dest.to_string_lossy()
    }));
    let res =
        tauri::test::get_ipc_response(&window, invoke_request("move_compressed_file", body));
    assert!(res.is_ok(), "move_compressed_file failed: {:?}", res.err());
    assert!(!source.exists());
    assert!(dest.exists());
    assert_eq!(fs::read(&dest).unwrap(), b"video data");
}

#[test]
fn cleanup_temp_file_removes_file() {
    let app = create_test_app();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("failed to create window");

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("temp.mp4");
    fs::write(&path, b"temp").unwrap();

    let body = InvokeBody::from(serde_json::json!({ "path": path.to_string_lossy() }));
    let res =
        tauri::test::get_ipc_response(&window, invoke_request("cleanup_temp_file", body));
    assert!(res.is_ok());
    assert!(!path.exists());
}
