use crate::codec::SUPPORTED_CODEC_NAMES;
use crate::error::AppError;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use tauri::utils::platform;

#[cfg(target_os = "windows")]
const FIND_CMD: &str = "where";
#[cfg(not(target_os = "windows"))]
const FIND_CMD: &str = "which";

fn find_in_path() -> Option<PathBuf> {
    let output = Command::new(FIND_CMD).arg("ffmpeg").output().ok()?;
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
    return vec![
        PathBuf::from("/opt/homebrew/bin/ffmpeg"),
        PathBuf::from("/usr/local/bin/ffmpeg"),
        PathBuf::from("/opt/local/bin/ffmpeg"),
    ];
    #[cfg(target_os = "windows")]
    return vec![
        PathBuf::from("C:\\ffmpeg\\bin\\ffmpeg.exe"),
        PathBuf::from("C:\\Program Files\\ffmpeg\\bin\\ffmpeg.exe"),
    ];
    #[cfg(all(unix, not(target_os = "macos")))]
    return vec![
        PathBuf::from("/usr/bin/ffmpeg"),
        PathBuf::from("/usr/local/bin/ffmpeg"),
    ];
    #[cfg(not(any(target_os = "macos", target_os = "windows", unix)))]
    return vec![];
}

#[cfg_attr(feature = "discovery-test-helpers", allow(dead_code))]
static FFMPEG_PATH_CACHE: OnceLock<PathBuf> = OnceLock::new();

/// Test-only: resettable cache so discovery tests can run in any order without reusing a previous test's path.
#[cfg(feature = "discovery-test-helpers")]
static TEST_FFMPEG_CACHE: parking_lot::Mutex<Option<&'static Path>> = parking_lot::Mutex::new(None);

#[cfg(feature = "discovery-test-helpers")]
pub fn __test_reset_ffmpeg_path_cache() {
    *TEST_FFMPEG_CACHE.lock() = None;
}

/// Resolve path to bundled sidecar (next to executable). macOS/Windows only.
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

/// Base names for bundled sidecar (suffixed first, then plain).
#[cfg(all(
    any(target_os = "macos", target_os = "windows"),
    not(feature = "lgpl-macos")
))]
fn bundled_sidecar_base_names() -> [&'static str; 2] {
    [concat!("ffmpeg-", env!("TARGET")), "ffmpeg"]
}

/// Resolve FFmpeg path. Order: bundled sidecar (if not lgpl-macos) → common paths → PATH → sidecar fallback.
fn resolve_ffmpeg_path() -> Result<PathBuf, AppError> {
    #[cfg(all(
        any(target_os = "macos", target_os = "windows"),
        not(feature = "lgpl-macos")
    ))]
    for base_name in bundled_sidecar_base_names() {
        if let Some(p) = resolve_sidecar_path(base_name) {
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
    if let Some(p) = find_in_path()
        && p.exists() {
            log::debug!(
                target: "tiny_vid::ffmpeg::discovery",
                "FFmpeg found in PATH: {}",
                p.display()
            );
            return Ok(p);
        }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
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
/// 1. FFMPEG_PATH env (when set and path exists) – for tests/CI or bundled binaries.
/// 2. When not lgpl-macos on macOS/Windows: bundled sidecar first (ffmpeg-{TARGET} then ffmpeg).
/// 3. Common installation paths (Homebrew, /usr/bin, etc.).
/// 4. PATH (via which/where).
/// 5. Bundled sidecar fallback (macOS/Windows only; used for lgpl when nothing in path).
pub fn get_ffmpeg_path() -> Result<&'static Path, AppError> {
    #[cfg(feature = "discovery-test-helpers")]
    {
        let guard = TEST_FFMPEG_CACHE.lock();
        if let Some(p) = *guard {
            return Ok(p);
        }
    }
    #[cfg(not(feature = "discovery-test-helpers"))]
    if let Some(path) = FFMPEG_PATH_CACHE.get() {
        log::trace!(
            target: "tiny_vid::ffmpeg::discovery",
            "FFmpeg path (cached): {}",
            path.display()
        );
        return Ok(path.as_path());
    }
    let path = match std::env::var("FFMPEG_PATH").ok().map(PathBuf::from) {
        Some(p) if p.exists() => {
            log::debug!(
                target: "tiny_vid::ffmpeg::discovery",
                "FFmpeg path from FFMPEG_PATH env: {}",
                p.display()
            );
            p
        }
        _ => resolve_ffmpeg_path()?,
    };
    #[cfg(feature = "discovery-test-helpers")]
    {
        let leaked: &'static Path = Box::leak(path.into_boxed_path());
        *TEST_FFMPEG_CACHE.lock() = Some(leaked);
        return Ok(leaked);
    }
    #[cfg(not(feature = "discovery-test-helpers"))]
    {
        let _ = FFMPEG_PATH_CACHE.set(path);
        Ok(FFMPEG_PATH_CACHE.get().unwrap().as_path())
    }
}

/// Paths to try for ffprobe given an ffmpeg binary path (suffixed first, then plain).
pub fn ffprobe_candidates(ffmpeg_path: &Path) -> Vec<PathBuf> {
    let parent = match ffmpeg_path.parent() {
        Some(p) => p,
        None => return vec![],
    };
    let mut candidates = Vec::with_capacity(2);
    let stem = ffmpeg_path.file_stem().and_then(|s| s.to_str());
    if let Some(stem) = stem
        && let Some(suffix) = stem.strip_prefix("ffmpeg")
            && !suffix.is_empty() {
                #[cfg(target_os = "windows")]
                candidates.push(parent.join(format!("ffprobe{suffix}.exe")));
                #[cfg(not(target_os = "windows"))]
                candidates.push(parent.join(format!("ffprobe{suffix}")));
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
    let candidates = ffprobe_candidates(ffmpeg);
    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }
    let expected = candidates
        .last()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| format!("ffprobe in {}", parent.display()));
    Err(AppError::from(format!(
        "ffprobe not found at {} (FFmpeg dir: {})",
        expected,
        parent.display()
    )))
}

/// Parse ffmpeg -encoders stdout and return supported video encoder names.
/// Lines starting with " V" are video encoders; we filter to codecs we support.
fn parse_encoder_output(stdout: &str) -> Vec<String> {
    let mut codecs = Vec::new();
    for line in stdout.lines() {
        if line.starts_with(" V")
            && let Some(codec_name) = line.split_whitespace().nth(1)
                && SUPPORTED_CODEC_NAMES.contains(&codec_name) {
                    codecs.push(codec_name.to_string());
                }
    }
    codecs
}

/// Detects available video encoders by running `ffmpeg -encoders`.
/// Returns list of codec names that we support (libx264, libx265, etc.).
pub fn get_available_codecs() -> Result<Vec<String>, AppError> {
    let ffmpeg_path = get_ffmpeg_path()?;
    log::debug!(
        target: "tiny_vid::ffmpeg::discovery",
        "Detecting available codecs from: {}",
        ffmpeg_path.display()
    );
    let output = Command::new(ffmpeg_path)
        .arg("-encoders")
        .output()
        .map_err(|e| AppError::from(format!("Failed to run ffmpeg -encoders: {}", e)))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::from(format!("ffmpeg -encoders failed: {}", stderr)));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let codecs = parse_encoder_output(&stdout);
    log::debug!(
        target: "tiny_vid::ffmpeg::discovery",
        "Detected {} supported codecs: {:?}",
        codecs.len(),
        codecs
    );
    Ok(codecs)
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

    #[test]
    fn parse_ffmpeg_encoders_output() {
        let sample_output = r#"
Encoders:
 V..... libx264              H.264 / AVC / MPEG-4 AVC / MPEG-4 part 10
 V..... libx265              H.265 / HEVC (High Efficiency Video Coding)
 V..... libsvtav1            SVT-AV1(Scalable Video Technology for AV1) encoder
 V..... libvpx-vp9           libvpx VP9
 V..... h264_videotoolbox    VideoToolbox H.264 Encoder
 V..... hevc_videotoolbox    VideoToolbox H.265 Encoder
 V..... mpeg4                MPEG-4 part 2
 A..... aac                  AAC (Advanced Audio Coding)
"#;
        let codecs = parse_encoder_output(sample_output);
        assert_eq!(codecs.len(), 6);
        assert!(codecs.contains(&"libx264".to_string()));
        assert!(codecs.contains(&"h264_videotoolbox".to_string()));
        assert!(!codecs.contains(&"mpeg4".to_string()));
        assert!(!codecs.contains(&"aac".to_string()));
    }
    
    #[test]
    #[ignore]
    fn get_available_codecs_returns_valid_list() {
        let result = get_available_codecs();
        assert!(result.is_ok(), "Should detect codecs: {:?}", result.err());
        let codecs = result.unwrap();
        assert!(!codecs.is_empty(), "Should detect at least one codec");
    }
}
