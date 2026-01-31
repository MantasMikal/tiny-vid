use crate::error::AppError;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

#[cfg(target_os = "windows")]
fn find_in_path() -> Option<PathBuf> {
    let output = Command::new("where").arg("ffmpeg").output().ok()?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout);
        let first = path.lines().next()?.trim();
        if !first.is_empty() {
            return Some(PathBuf::from(first));
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
fn find_in_path() -> Option<PathBuf> {
    let output = Command::new("which").arg("ffmpeg").output().ok()?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout);
        let first = path.lines().next()?.trim();
        if !first.is_empty() {
            return Some(PathBuf::from(first));
        }
    }
    None
}

fn common_paths() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        vec![
            PathBuf::from("/opt/homebrew/bin/ffmpeg"),
            PathBuf::from("/usr/local/bin/ffmpeg"),
            PathBuf::from("/opt/local/bin/ffmpeg"),
        ]
    }

    #[cfg(target_os = "windows")]
    {
        vec![
            PathBuf::from("C:\\ffmpeg\\bin\\ffmpeg.exe"),
            PathBuf::from("C:\\Program Files\\ffmpeg\\bin\\ffmpeg.exe"),
        ]
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        vec![
            PathBuf::from("/usr/bin/ffmpeg"),
            PathBuf::from("/usr/local/bin/ffmpeg"),
        ]
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", unix)))]
    {
        vec![]
    }
}

static FFMPEG_PATH_CACHE: OnceLock<PathBuf> = OnceLock::new();

fn resolve_ffmpeg_path() -> Result<PathBuf, AppError> {
    if let Some(p) = find_in_path() {
        if p.exists() {
            log::debug!(
                target: "tiny_vid::ffmpeg::discovery",
                "FFmpeg found in PATH: {}",
                p.display()
            );
            return Ok(p);
        }
    }

    for path in common_paths() {
        if path.exists() {
            log::debug!(
                target: "tiny_vid::ffmpeg::discovery",
                "FFmpeg found in common path: {}",
                path.display()
            );
            return Ok(path);
        }
    }

    log::error!(
        target: "tiny_vid::ffmpeg::discovery",
        "FFmpeg not found in PATH or common locations"
    );
    Err(AppError::FfmpegNotFound(
        "FFmpeg not found. Please install FFmpeg on your system:\n  - macOS: brew install ffmpeg\n  - Linux: sudo apt install ffmpeg\n  - Windows: Download from https://ffmpeg.org/download.html"
            .to_string(),
    ))
}

/// Get FFmpeg path. Cached for process lifetime.
/// Env override: FFMPEG_PATH takes precedence (for tests/CI or bundled binaries).
/// Falls back to PATH, then common installation paths.
pub fn get_ffmpeg_path() -> Result<&'static Path, AppError> {
    if let Some(path) = FFMPEG_PATH_CACHE.get() {
        log::trace!(
            target: "tiny_vid::ffmpeg::discovery",
            "FFmpeg path (cached): {}",
            path.display()
        );
        return Ok(path.as_path());
    }
    let path = if let Ok(env_path) = std::env::var("FFMPEG_PATH") {
        let p = PathBuf::from(&env_path);
        if p.exists() {
            log::debug!(
                target: "tiny_vid::ffmpeg::discovery",
                "FFmpeg path from FFMPEG_PATH env: {}",
                p.display()
            );
            p
        } else {
            resolve_ffmpeg_path()?
        }
    } else {
        resolve_ffmpeg_path()?
    };
    match FFMPEG_PATH_CACHE.set(path) {
        Ok(()) => {}
        Err(_) => {} // Another thread initialized first
    }
    Ok(FFMPEG_PATH_CACHE.get().unwrap().as_path())
}
