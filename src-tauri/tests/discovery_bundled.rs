//! Integration tests for FFmpeg discovery mechanisms.
//! Runs in its own process so FFMPEG_PATH and the discovery cache are clean.

use std::env;
use std::fs;
use tiny_vid_tauri_lib::ffmpeg::discovery::{get_ffmpeg_path, get_ffprobe_path, resolve_sidecar_path};
#[cfg(feature = "discovery-test-helpers")]
use tiny_vid_tauri_lib::ffmpeg::discovery::__test_reset_ffmpeg_path_cache;

/// Derive bundled suffix from target at compile time.
fn bundled_suffix() -> String {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    if cfg!(windows) {
        format!("{}-pc-windows-msvc.exe", arch)
    } else if cfg!(target_os = "macos") {
        let target = if arch == "aarch64" {
            "aarch64-apple-darwin"
        } else {
            "x86_64-apple-darwin"
        };
        target.to_string()
    } else {
        format!("{}-unknown-{}", arch, os)
    }
}

/// FFMPEG_PATH override with suffixed binary; get_ffprobe_path derives matching ffprobe.
#[test]
fn env_var_override_with_suffixed_binaries() {
    let dir = env::temp_dir().join("tiny_vid_discovery_test").join(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string(),
    );
    fs::create_dir_all(&dir).expect("create temp dir");

    let suffix = bundled_suffix();
    let ffmpeg_name = format!("ffmpeg-{}", suffix);
    let ffprobe_name = format!("ffprobe-{}", suffix);

    let ffmpeg_path = dir.join(&ffmpeg_name);
    let ffprobe_path = dir.join(&ffprobe_name);
    fs::File::create(&ffmpeg_path).expect("create mock ffmpeg");
    fs::File::create(&ffprobe_path).expect("create mock ffprobe");

    let previous = env::var("FFMPEG_PATH").ok();
    unsafe { env::set_var("FFMPEG_PATH", &ffmpeg_path) };
    let _guard = RestoreEnv {
        key: "FFMPEG_PATH".to_string(),
        previous,
    };

    let got_ffmpeg = get_ffmpeg_path().expect("get_ffmpeg_path should succeed");
    let got_ffprobe = get_ffprobe_path().expect("get_ffprobe_path should succeed");

    assert_eq!(
        got_ffmpeg,
        ffmpeg_path.as_path(),
        "get_ffmpeg_path should return the bundled ffmpeg path"
    );
    assert_eq!(
        got_ffprobe, ffprobe_path,
        "get_ffprobe_path should return the bundled ffprobe path (same suffix as ffmpeg)"
    );

    // Best-effort cleanup
    let _ = fs::remove_file(&ffmpeg_path);
    let _ = fs::remove_file(&ffprobe_path);
    let _ = fs::remove_dir(dir.parent().unwrap());
    let _ = fs::remove_dir(dir.parent().unwrap().parent().unwrap());
}

/// Prefer bundled sidecar when not lgpl-macos. Run: cargo test --test discovery_bundled --features discovery-test-helpers
#[test]
#[cfg(all(
    any(target_os = "macos", target_os = "windows"),
    not(feature = "lgpl-macos"),
    feature = "discovery-test-helpers"
))]
fn prefer_bundled_sidecar_when_not_lgpl() {
    __test_reset_ffmpeg_path_cache();
    let exe_path = env::current_exe().expect("get current exe path");
    let exe_dir = exe_path.parent().expect("exe parent dir");

    let target = env!("TARGET");
    #[cfg(windows)]
    let (ffmpeg_name, ffprobe_name) = (
        format!("ffmpeg-{}.exe", target),
        format!("ffprobe-{}.exe", target),
    );
    #[cfg(not(windows))]
    let (ffmpeg_name, ffprobe_name) = (
        format!("ffmpeg-{}", target),
        format!("ffprobe-{}", target),
    );

    let mock_ffmpeg = exe_dir.join(&ffmpeg_name);
    let mock_ffprobe = exe_dir.join(&ffprobe_name);

    let _ = fs::remove_file(&mock_ffmpeg);
    let _ = fs::remove_file(&mock_ffprobe);

    fs::File::create(&mock_ffmpeg).expect("create mock ffmpeg sidecar");
    fs::File::create(&mock_ffprobe).expect("create mock ffprobe sidecar");

    let previous = env::var("FFMPEG_PATH").ok();
    unsafe { env::remove_var("FFMPEG_PATH") };
    let _guard = RestoreEnv {
        key: "FFMPEG_PATH".to_string(),
        previous,
    };

    let got_ffmpeg = get_ffmpeg_path().expect("get_ffmpeg_path should succeed");
    let got_ffprobe = get_ffprobe_path().expect("get_ffprobe_path should succeed");

    let _ = fs::remove_file(&mock_ffmpeg);
    let _ = fs::remove_file(&mock_ffprobe);

    assert_eq!(
        got_ffmpeg,
        mock_ffmpeg.as_path(),
        "get_ffmpeg_path should return the suffixed bundled sidecar when not lgpl-macos"
    );
    assert_eq!(
        got_ffprobe,
        mock_ffprobe,
        "get_ffprobe_path should return the suffixed bundled ffprobe"
    );
}

/// resolve_sidecar_path finds binaries next to the current executable.
#[test]
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn discovers_sidecar_binaries() {
    let exe_path = env::current_exe().expect("get current exe path");
    let exe_dir = exe_path.parent().expect("get exe parent dir");

    #[cfg(windows)]
    let ffmpeg_name = "ffmpeg.exe";
    #[cfg(not(windows))]
    let ffmpeg_name = "ffmpeg";

    let mock_ffmpeg = exe_dir.join(ffmpeg_name);

    let _ = fs::remove_file(&mock_ffmpeg);
    fs::File::create(&mock_ffmpeg).expect("create mock ffmpeg next to test binary");

    let result = resolve_sidecar_path("ffmpeg");
    let _ = fs::remove_file(&mock_ffmpeg);

    assert!(
        result.is_some(),
        "resolve_sidecar_path should find ffmpeg next to executable"
    );
    assert_eq!(
        result.unwrap(),
        mock_ffmpeg,
        "resolve_sidecar_path should return the correct path"
    );
}

/// resolve_sidecar_path returns None when binary doesn't exist.
#[test]
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn sidecar_returns_none_when_missing() {
    let result = resolve_sidecar_path("ffmpeg_nonexistent_test_binary_12345");
    assert!(
        result.is_none(),
        "resolve_sidecar_path should return None when binary doesn't exist"
    );
}

/// resolve_sidecar_path returns None on Linux (no sidecar bundling).
#[test]
#[cfg(target_os = "linux")]
fn sidecar_returns_none_on_linux() {
    let result = resolve_sidecar_path("ffmpeg");
    assert!(
        result.is_none(),
        "resolve_sidecar_path should return None on Linux (no bundle)"
    );
}

/// Smoke test: when FFMPEG_PATH points to a real bundled ffmpeg binary, run -version.
/// Run after build: `FFMPEG_PATH=path/to/bundled/ffmpeg cargo test --test discovery_bundled bundled_ffmpeg_version -- --ignored`
#[test]
#[ignore = "run after build with FFMPEG_PATH pointing to bundled ffmpeg"]
fn bundled_ffmpeg_version() {
    let ffmpeg_path = env::var("FFMPEG_PATH").expect("FFMPEG_PATH must be set for this test");
    let path = std::path::PathBuf::from(&ffmpeg_path);
    assert!(path.exists(), "FFMPEG_PATH must point to an existing file: {}", ffmpeg_path);

    let output = std::process::Command::new(&path)
        .arg("-version")
        .output()
        .expect("failed to run ffmpeg -version");
    assert!(
        output.status.success(),
        "ffmpeg -version failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ffmpeg") || stdout.contains("FFmpeg"),
        "expected version output, got: {}",
        stdout
    );
}

/// Restore an env var to its previous value when dropped.
struct RestoreEnv {
    key: String,
    previous: Option<String>,
}

impl Drop for RestoreEnv {
    fn drop(&mut self) {
        match &self.previous {
            Some(v) => unsafe { env::set_var(&self.key, v) },
            None => unsafe { env::remove_var(&self.key) },
        }
    }
}
