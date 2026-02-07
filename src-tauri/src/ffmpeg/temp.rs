//! Temp file management and cleanup for FFmpeg operations.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::cache::get_all_cached_paths;
use parking_lot::Mutex;

static PREVIOUS_PREVIEW_PATHS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
static TRANSCODE_TEMP_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);
const TEMP_FILE_PREFIX: &str = "tiny-vid-";

/// Set the current transcode temp path (for cleanup on exit or cancel).
pub fn set_transcode_temp(path: Option<PathBuf>) {
    let mut guard = TRANSCODE_TEMP_PATH.lock();
    *guard = path;
}

/// Remove the transcode temp file if it exists. Call on app exit or when user cancels save.
pub fn cleanup_transcode_temp() {
    let mut guard = TRANSCODE_TEMP_PATH.lock();
    if let Some(path) = guard.take() {
        log::debug!(
            target: "tiny_vid::ffmpeg::temp",
            "cleanup_transcode_temp: removing {}",
            path.display()
        );
        let _ = fs::remove_file(&path);
    }
}

/// Delete temp files from the previous preview. Call at the start of each new preview.
/// Preserves any paths that are still referenced by the preview cache.
pub fn cleanup_previous_preview_paths(_new_input_path: &str, _new_preview_duration: u32) {
    let mut guard = PREVIOUS_PREVIEW_PATHS.lock();
    let paths: Vec<_> = guard.drain(..).collect();

    let paths_to_keep = get_all_cached_paths();

    for path in &paths {
        if paths_to_keep.iter().any(|keep| keep == path) {
            log::trace!(
                target: "tiny_vid::ffmpeg::temp",
                "cleanup_previous_preview_paths: keeping cached path {}",
                path.display()
            );
            continue;
        }
        log::trace!(
            target: "tiny_vid::ffmpeg::temp",
            "cleanup_previous_preview_paths: removing {}",
            path.display()
        );
        let _ = fs::remove_file(path);
    }
}

/// Store paths to be cleaned up when the next preview is generated.
pub fn store_preview_paths_for_cleanup(originals: &[PathBuf], compresseds: &[PathBuf]) {
    log::debug!(
        target: "tiny_vid::ffmpeg::temp",
        "store_preview_paths_for_cleanup: {} originals, {} compresseds",
        originals.len(),
        compresseds.len()
    );
    let mut guard = PREVIOUS_PREVIEW_PATHS.lock();
    guard.extend(originals.iter().cloned());
    guard.extend(compresseds.iter().cloned());
}

/// Stateless factory for creating temp files. Paths must be handed off to
/// `set_transcode_temp` or `store_preview_paths_for_cleanup` for cleanup.
pub struct TempFileManager;

impl Default for TempFileManager {
    fn default() -> Self {
        Self
    }
}

/// Generates a short random suffix for temp filenames
fn random_alphanumeric_suffix(len: usize) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    static STATE: AtomicU64 = AtomicU64::new(0);
    let mut s = String::with_capacity(len);
    for _ in 0..len {
        let idx = STATE.fetch_add(1, Ordering::Relaxed) as usize % CHARS.len();
        s.push(CHARS[idx] as char);
    }
    s
}

impl TempFileManager {
    pub fn create(&self, suffix: &str, content: Option<&[u8]>) -> io::Result<PathBuf> {
        let tmp = std::env::temp_dir();
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before UNIX_EPOCH")
            .as_millis();
        let name = format!(
            "{}{}-{}-{}",
            TEMP_FILE_PREFIX,
            timestamp_ms,
            random_alphanumeric_suffix(9),
            suffix
        );
        let path = tmp.join(name);
        if let Some(data) = content {
            fs::write(&path, data)?;
        }
        log::debug!(
            target: "tiny_vid::ffmpeg::temp",
            "TempFileManager::create: suffix={}, path={}",
            suffix,
            path.display()
        );
        Ok(path)
    }
}

/// Best-effort cleanup of old temp files on startup.
/// Deletes files matching `tiny-vid-{timestamp}-...` older than `max_age`.
pub fn cleanup_old_temp_files(max_age: Duration) {
    let tmp = std::env::temp_dir();
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let max_age_ms = max_age.as_millis();

    let entries = match fs::read_dir(&tmp) {
        Ok(entries) => entries,
        Err(e) => {
            log::debug!(
                target: "tiny_vid::ffmpeg::temp",
                "cleanup_old_temp_files: failed to read temp dir {}: {}",
                tmp.display(),
                e
            );
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => continue,
        };
        let Some(ts_ms) = parse_timestamp_from_name(file_name) else {
            continue;
        };
        let age_ms = now_ms.saturating_sub(ts_ms);
        if age_ms > max_age_ms {
            log::trace!(
                target: "tiny_vid::ffmpeg::temp",
                "cleanup_old_temp_files: removing stale temp file {} (age_ms={})",
                path.display(),
                age_ms
            );
            let _ = fs::remove_file(&path);
        }
    }
}

fn parse_timestamp_from_name(name: &str) -> Option<u128> {
    let rest = name.strip_prefix(TEMP_FILE_PREFIX)?;
    let ts = rest.split('-').next()?;
    ts.parse::<u128>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_returns_path_under_temp_dir_with_suffix() {
        let manager = TempFileManager::default();
        let path = manager.create("suffix.mp4", None).unwrap();
        let tmp = std::env::temp_dir();
        assert!(
            path.starts_with(&tmp),
            "path {:?} should be under temp_dir {:?}",
            path,
            tmp
        );
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with("suffix.mp4"),
            "file name should end with suffix: {:?}",
            path.file_name()
        );
        assert!(!path.exists(), "create(_, None) should not create a file");
    }

    #[test]
    fn create_with_content_writes_file() {
        let manager = TempFileManager::default();
        let data = b"video data";
        let path = manager.create("test.mp4", Some(data)).unwrap();
        assert!(path.exists());
        assert_eq!(fs::read(&path).unwrap(), data);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn create_yields_different_paths() {
        let manager = TempFileManager::default();
        let path1 = manager.create("x", None).unwrap();
        let path2 = manager.create("x", None).unwrap();
        assert_ne!(
            path1, path2,
            "two create calls should yield different paths"
        );
    }
}
