//! Video integrity verification via FFmpeg decode-to-null.
//!
//! A valid video decodes without errors; corruption produces FFmpeg errors and non-zero exit.
//! For AV1, uses libdav1d (same as VLC/QuickTime) to catch SVT-AV1 compatibility issues.

use std::path::Path;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use super::discovery::get_ffmpeg_path;

fn run_verify(ffmpeg: &std::path::Path, path_str: &str, use_dav1d: bool) -> (bool, i32, String) {
    let args: Vec<&str> = if use_dav1d {
        vec!["-v", "error", "-c:v", "libdav1d", "-i", path_str, "-f", "null", "-"]
    } else {
        vec!["-v", "error", "-i", path_str, "-f", "null", "-"]
    };
    let mut cmd = Command::new(ffmpeg);
    cmd.args(&args);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => return (false, -1, e.to_string()),
    };
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let exit_code = output.status.code().unwrap_or(-1);
    let success = output.status.success() && !stderr.to_lowercase().contains("error");
    (success, exit_code, stderr)
}

fn is_dav1d_unavailable(stderr: &str) -> bool {
    let s = stderr.to_lowercase();
    s.contains("unknown decoder")
        || s.contains("decoder 'libdav1d' not found")
        || s.contains("could not find decoder")
        || s.contains("no decoder for")
}

/// Run FFmpeg decode-to-null. Returns Ok(()) if decode succeeds without errors.
/// For AV1, uses libdav1d (falls back to default if unavailable). For non-AV1, uses default decoder.
#[allow(dead_code)] // Used by integration tests; may be used for runtime verification
pub fn verify_video(path: &Path, codec: Option<&str>) -> Result<(), String> {
    let ffmpeg = get_ffmpeg_path().map_err(|e| e.to_string())?;
    let path_str = path.to_string_lossy();
    let use_dav1d = codec
        .map(|c| c.to_lowercase().contains("svtav1") || c.to_lowercase().contains("av1"))
        .unwrap_or(false);

    let (success, exit_code, stderr) =
        run_verify(ffmpeg, path_str.as_ref(), use_dav1d);

    if success {
        return Ok(());
    }
    if use_dav1d && is_dav1d_unavailable(&stderr) {
        let (fallback_success, fallback_code, fallback_stderr) =
            run_verify(ffmpeg, path_str.as_ref(), false);
        if fallback_success {
            return Ok(());
        }
        return Err(format!(
            "Video verification failed (exit {}): {}",
            fallback_code, fallback_stderr
        ));
    }
    Err(format!(
        "Video verification failed (exit {}): {}",
        exit_code, stderr
    ))
}
