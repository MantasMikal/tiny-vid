//! Tauri IPC command tests. Uses test_util for app and invoke helpers.

use crate::test_util::{
    create_test_app, create_test_app_with_file_assoc, create_test_video,
    create_test_video_with_multi_audio, find_ffmpeg_and_set_env, invoke_request,
};
use crate::CodecInfo;
use std::fs;
use tauri::ipc::InvokeBody;

#[test]
fn get_build_variant_returns_variant_and_codecs() {
    let app = create_test_app();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("failed to create window");

    let body = InvokeBody::default();
    let res = tauri::test::get_ipc_response(&window, invoke_request("get_build_variant", body));
    assert!(res.is_ok(), "get_build_variant failed: {:?}", res.err());
    let body = res.unwrap();
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct BuildVariantResult {
        variant: String,
        codecs: Vec<CodecInfo>,
    }
    let result: BuildVariantResult = body.deserialize().unwrap();
    assert!(!result.codecs.is_empty(), "codecs should not be empty");

    for codec in &result.codecs {
        assert!(!codec.value.is_empty());
        assert!(!codec.name.is_empty());
        assert!(!codec.formats.is_empty());
    }

    #[cfg(feature = "lgpl")]
    {
        assert_eq!(result.variant, "lgpl", "lgpl build should return variant lgpl");
    }

    #[cfg(not(feature = "lgpl"))]
    {
        assert_eq!(result.variant, "standalone", "standalone build should return variant standalone");
    }
}

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
fn get_video_metadata_nonexistent_returns_error() {
    let app = create_test_app();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("failed to create window");

    let body = InvokeBody::from(serde_json::json!({
        "path": "/nonexistent/path/video.mp4"
    }));
    let res = tauri::test::get_ipc_response(&window, invoke_request("get_video_metadata", body));
    assert!(res.is_err(), "get_video_metadata should fail for nonexistent path");
}

#[test]
#[ignore = "requires FFmpeg/ffprobe on system; run with: cargo test get_video_metadata_with_video -- --ignored"]
fn get_video_metadata_with_video_returns_metadata() {
    let ffmpeg = find_ffmpeg_and_set_env();

    let app = create_test_app();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("failed to create window");

    let dir = tempfile::tempdir().unwrap();
    let video_path = dir.path().join("test.mp4");
    let status = create_test_video(&ffmpeg, &video_path, 2.0).expect("failed to create test video");
    assert!(status.success(), "ffmpeg failed to create test video");

    let body = InvokeBody::from(serde_json::json!({
        "path": video_path.to_string_lossy()
    }));
    let res =
        tauri::test::get_ipc_response(&window, invoke_request("get_video_metadata", body));
    assert!(res.is_ok(), "get_video_metadata failed: {:?}", res.err());

    let body = res.unwrap();
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Meta {
        duration: f64,
        width: u32,
        height: u32,
        size: u64,
    }
    let meta: Meta = body.deserialize().unwrap();
    assert!(meta.duration > 1.0 && meta.duration < 3.0, "duration={}", meta.duration);
    assert_eq!(meta.width, 320);
    assert_eq!(meta.height, 240);
    assert!(meta.size > 0);
}

#[test]
#[ignore = "requires FFmpeg on system; run with: cargo test get_video_metadata_multi_audio_returns_audio_stream_count -- --ignored"]
fn get_video_metadata_multi_audio_returns_audio_stream_count() {
    let ffmpeg = find_ffmpeg_and_set_env();

    let app = create_test_app();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("failed to create window");

    let dir = tempfile::tempdir().unwrap();
    let video_path = dir.path().join("multi_audio.mp4");
    let status = create_test_video_with_multi_audio(&ffmpeg, &video_path, 2.0, 2)
        .expect("failed to create test video with multi audio");
    assert!(status.success(), "ffmpeg failed to create multi-audio test video");

    let body = InvokeBody::from(serde_json::json!({
        "path": video_path.to_string_lossy()
    }));
    let res =
        tauri::test::get_ipc_response(&window, invoke_request("get_video_metadata", body));
    assert!(res.is_ok(), "get_video_metadata failed: {:?}", res.err());

    let body = res.unwrap();
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Meta {
        duration: f64,
        width: u32,
        height: u32,
        size: u64,
        audio_stream_count: u32,
    }
    let meta: Meta = body.deserialize().unwrap();
    assert!(meta.duration > 1.0 && meta.duration < 3.0, "duration={}", meta.duration);
    assert_eq!(meta.width, 320);
    assert_eq!(meta.height, 240);
    assert!(meta.size > 0);
    assert_eq!(
        meta.audio_stream_count, 2,
        "audioStreamCount should be 2 for multi-audio file"
    );
}

#[test]
fn get_pending_opened_files_returns_empty_when_buffer_empty() {
    let app = create_test_app_with_file_assoc(None);
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("failed to create window");

    let body = InvokeBody::default();
    let res = tauri::test::get_ipc_response(
        &window,
        invoke_request("get_pending_opened_files", body),
    );
    assert!(res.is_ok(), "get_pending_opened_files failed: {:?}", res.err());
    let body = res.unwrap();
    let paths: Vec<String> = body.deserialize().unwrap();
    assert!(paths.is_empty(), "expected empty vec, got {:?}", paths);
}

#[test]
fn get_pending_opened_files_returns_and_clears_buffered_paths() {
    let dir = tempfile::tempdir().unwrap();
    let path_a = dir.path().join("a.mp4");
    let path_b = dir.path().join("b.mp4");
    let paths: Vec<std::path::PathBuf> = vec![path_a.clone(), path_b.clone()];

    let app = create_test_app_with_file_assoc(Some(paths));
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("failed to create window");

    let body = InvokeBody::default();
    let res = tauri::test::get_ipc_response(
        &window,
        invoke_request("get_pending_opened_files", body.clone()),
    );
    assert!(res.is_ok(), "first invoke failed: {:?}", res.err());
    let first: Vec<String> = res.unwrap().deserialize().unwrap();
    assert_eq!(
        first,
        vec![
            path_a.to_string_lossy().to_string(),
            path_b.to_string_lossy().to_string(),
        ],
        "first invoke should return buffered paths"
    );

    let res = tauri::test::get_ipc_response(
        &window,
        invoke_request("get_pending_opened_files", body),
    );
    assert!(res.is_ok(), "second invoke failed: {:?}", res.err());
    let second: Vec<String> = res.unwrap().deserialize().unwrap();
    assert!(second.is_empty(), "second invoke should return empty (buffer drained)");
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
