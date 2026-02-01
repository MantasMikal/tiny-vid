//! Tauri IPC command tests. Uses test_util for app and invoke helpers.

use crate::test_util::{create_test_app, invoke_request};
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
        codecs: Vec<String>,
    }
    let result: BuildVariantResult = body.deserialize().unwrap();
    assert!(!result.codecs.is_empty(), "codecs should not be empty");

    #[cfg(feature = "lgpl-macos")]
    {
        assert_eq!(result.variant, "lgpl-macos", "lgpl-macos build should return variant lgpl-macos");
        assert!(
            result.codecs.contains(&"h264_videotoolbox".to_string()),
            "lgpl-macos codecs should include h264_videotoolbox, got {:?}",
            result.codecs
        );
        assert!(
            result.codecs.contains(&"hevc_videotoolbox".to_string()),
            "lgpl-macos codecs should include hevc_videotoolbox, got {:?}",
            result.codecs
        );
    }

    #[cfg(not(feature = "lgpl-macos"))]
    {
        assert_eq!(result.variant, "full", "full build should return variant full");
        assert!(
            result.codecs.contains(&"libx264".to_string()),
            "full codecs should include libx264, got {:?}",
            result.codecs
        );
        assert!(
            result.codecs.contains(&"libx265".to_string()),
            "full codecs should include libx265, got {:?}",
            result.codecs
        );
        assert!(
            result.codecs.contains(&"libsvtav1".to_string()),
            "full codecs should include libsvtav1, got {:?}",
            result.codecs
        );
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
    use std::process::Command;

    let ffmpeg = std::env::var("FFMPEG_PATH")
        .ok()
        .map(std::path::PathBuf::from)
        .filter(|p| p.exists())
        .or_else(|| {
            let cmd = if cfg!(windows) { "where" } else { "which" };
            let output = Command::new(cmd).arg("ffmpeg").output().ok()?;
            if output.status.success() {
                let first = std::str::from_utf8(&output.stdout)
                    .ok()?
                    .lines()
                    .next()?
                    .trim();
                if !first.is_empty() {
                    return Some(std::path::PathBuf::from(first));
                }
            }
            None
        })
        .expect("FFmpeg not found; set FFMPEG_PATH or add to PATH");

    unsafe {
        std::env::set_var("FFMPEG_PATH", ffmpeg.to_string_lossy().as_ref());
    }

    let app = create_test_app();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("failed to create window");

    let dir = tempfile::tempdir().unwrap();
    let video_path = dir.path().join("test.mp4");
    let status = {
        #[cfg(not(feature = "lgpl-macos"))]
        {
            Command::new(&ffmpeg)
                .args([
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc=duration=2:size=320x240:rate=30",
                    "-c:v",
                    "libx264",
                    "-pix_fmt",
                    "yuv420p",
                    video_path.to_str().unwrap(),
                ])
                .status()
        }
        #[cfg(feature = "lgpl-macos")]
        {
            Command::new(&ffmpeg)
                .args([
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc=duration=2:size=320x240:rate=30",
                    "-c:v",
                    "h264_videotoolbox",
                    "-allow_sw",
                    "1",
                    "-q:v",
                    "25",
                    video_path.to_str().unwrap(),
                ])
                .status()
        }
    }
    .expect("failed to create test video");
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
