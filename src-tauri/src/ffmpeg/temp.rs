use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;

static PREVIOUS_PREVIEW_PATHS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
static TRANSCODE_TEMP_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Set the current transcode temp path (for cleanup on exit or cancel).
pub fn set_transcode_temp(path: Option<PathBuf>) {
    if let Ok(mut guard) = TRANSCODE_TEMP_PATH.lock() {
        *guard = path;
    }
}

/// Remove the transcode temp file if it exists. Call on app exit or when user cancels save.
pub fn cleanup_transcode_temp() {
    let mut guard = TRANSCODE_TEMP_PATH.lock().unwrap_or_else(|e| e.into_inner());
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
pub fn cleanup_previous_preview_paths() {
    let mut guard = PREVIOUS_PREVIEW_PATHS.lock().unwrap_or_else(|e| e.into_inner());
    let paths: Vec<_> = guard.drain(..).collect();
    for path in &paths {
        log::trace!(
            target: "tiny_vid::ffmpeg::temp",
            "cleanup_previous_preview_paths: removing {}",
            path.display()
        );
        let _ = fs::remove_file(path);
    }
}

/// Store paths to be cleaned up when the next preview is generated.
pub fn store_preview_paths_for_cleanup(original: PathBuf, compressed: PathBuf) {
    log::debug!(
        target: "tiny_vid::ffmpeg::temp",
        "store_preview_paths_for_cleanup: original={}, compressed={}",
        original.display(),
        compressed.display()
    );
    if let Ok(mut guard) = PREVIOUS_PREVIEW_PATHS.lock() {
        guard.push(original);
        guard.push(compressed);
    }
}

/// Stateless factory for creating temp files. Paths must be handed off to
/// `set_transcode_temp` or `store_preview_paths_for_cleanup` for cleanup.
pub struct TempFileManager;

impl Default for TempFileManager {
    fn default() -> Self {
        Self
    }
}

/// Generates a short random suffix for temp filenames. Not cryptographically secure; for uniqueness only.
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
        let name = format!(
            "ffmpeg-{}-{}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before UNIX_EPOCH")
                .as_millis(),
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
            path.file_name().unwrap().to_string_lossy().ends_with("suffix.mp4"),
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
        assert_ne!(path1, path2, "two create calls should yield different paths");
    }
}
