use crate::error::AppError;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use tauri::utils::platform;

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

/// Resolve path to bundled sidecar binary (next to executable). macOS/Windows only.
/// Returns the path if the binary exists, None otherwise.
pub fn resolve_sidecar_path(base_name: &str) -> Option<PathBuf> {
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = base_name;
        return None;
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let exe_dir = platform::current_exe().ok()?.parent()?.to_path_buf();
        #[cfg(windows)]
        let path = {
            let base = base_name.trim_end_matches(".exe");
            let mut p = exe_dir.join(base);
            if !p.extension().is_some_and(|e| e == "exe") {
                p.as_mut_os_string().push(".exe");
            }
            p
        };
        #[cfg(not(windows))]
        let path = {
            let mut p = exe_dir.join(base_name);
            if p.extension().is_some_and(|e| e == "exe") {
                p.set_extension("");
            }
            p
        };
        if path.exists() {
            log::debug!(
                target: "tiny_vid::ffmpeg::discovery",
                "FFmpeg found as bundled sidecar: {}",
                path.display()
            );
            Some(path)
        } else {
            None
        }
    }
}

fn resolve_ffmpeg_path() -> Result<PathBuf, AppError> {
    // 1. Pre-installed: common paths first to avoid spawning which/where
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

    // 2. Pre-installed: PATH
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

    // 3. Bundled sidecar (macOS/Windows only; fallback for zero-install)
    if let Some(p) = resolve_sidecar_path("ffmpeg") {
        return Ok(p);
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

/// Paths to try for ffprobe given an ffmpeg binary path (suffixed first, then plain).
/// Used so we can unit-test the derivation logic.
pub fn ffprobe_candidates(ffmpeg_path: &Path) -> Vec<PathBuf> {
    let parent = match ffmpeg_path.parent() {
        Some(p) => p,
        None => return vec![],
    };
    let mut candidates = Vec::with_capacity(2);
    let stem = ffmpeg_path.file_stem().and_then(|s| s.to_str());
    if let Some(stem) = stem {
        if let Some(suffix) = stem.strip_prefix("ffmpeg") {
            if !suffix.is_empty() {
                #[cfg(target_os = "windows")]
                candidates.push(parent.join(format!("ffprobe{suffix}.exe")));
                #[cfg(not(target_os = "windows"))]
                candidates.push(parent.join(format!("ffprobe{suffix}")));
            }
        }
    }
    #[cfg(target_os = "windows")]
    candidates.push(parent.join("ffprobe.exe"));
    #[cfg(not(target_os = "windows"))]
    candidates.push(parent.join("ffprobe"));
    candidates
}

/// Get ffprobe path. Same directory as ffmpeg (ffmpeg/ffprobe ship together).
/// If ffmpeg has a platform suffix (e.g. ffmpeg-aarch64-apple-darwin), looks for
/// ffprobe with the same suffix (ffprobe-aarch64-apple-darwin) first.
pub fn get_ffprobe_path() -> Result<PathBuf, AppError> {
    let ffmpeg = get_ffmpeg_path()?;
    let parent = ffmpeg
        .parent()
        .ok_or_else(|| AppError::from("FFmpeg path has no parent directory".to_string()))?;
    for candidate in ffprobe_candidates(ffmpeg) {
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    #[cfg(target_os = "windows")]
    let ffprobe = parent.join("ffprobe.exe");
    #[cfg(not(target_os = "windows"))]
    let ffprobe = parent.join("ffprobe");
    Err(AppError::from(format!(
        "ffprobe not found at {} (FFmpeg dir: {})",
        ffprobe.display(),
        parent.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn ffprobe_candidates_plain_ffmpeg() {
        #[cfg(not(target_os = "windows"))]
        {
            let candidates = ffprobe_candidates(Path::new("/usr/bin/ffmpeg"));
            assert_eq!(candidates.len(), 1);
            assert_eq!(candidates[0], PathBuf::from("/usr/bin/ffprobe"));
        }
        #[cfg(target_os = "windows")]
        {
            let candidates = ffprobe_candidates(Path::new("C:\\bin\\ffmpeg.exe"));
            assert_eq!(candidates.len(), 1);
            assert_eq!(candidates[0], PathBuf::from("C:\\bin\\ffprobe.exe"));
        }
    }

    #[test]
    fn ffprobe_candidates_bundled_suffix_unix() {
        #[cfg(not(target_os = "windows"))]
        {
            let candidates =
                ffprobe_candidates(Path::new("/app/bin/ffmpeg-aarch64-apple-darwin"));
            assert_eq!(candidates.len(), 2);
            assert_eq!(
                candidates[0],
                PathBuf::from("/app/bin/ffprobe-aarch64-apple-darwin")
            );
            assert_eq!(candidates[1], PathBuf::from("/app/bin/ffprobe"));
        }
    }

    #[test]
    fn ffprobe_candidates_bundled_suffix_windows() {
        #[cfg(target_os = "windows")]
        {
            let candidates = ffprobe_candidates(Path::new(
                "C:\\app\\bin\\ffmpeg-x86_64-pc-windows-msvc.exe",
            ));
            assert_eq!(candidates.len(), 2);
            assert_eq!(
                candidates[0],
                PathBuf::from("C:\\app\\bin\\ffprobe-x86_64-pc-windows-msvc.exe")
            );
            assert_eq!(candidates[1], PathBuf::from("C:\\app\\bin\\ffprobe.exe"));
        }
    }
}
