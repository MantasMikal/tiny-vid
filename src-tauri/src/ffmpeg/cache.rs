//! Extract and transcode caches for preview generation.

use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use parking_lot::Mutex;
use super::TranscodeOptions;

const PREVIEW_TRANSCODE_CACHE_MAX_ENTRIES: usize = 16;

/// Key for the transcode cache: (input_path, preview_duration, options_key).
#[derive(Clone, Hash, Eq, PartialEq, Debug)]
struct TranscodeCacheKey {
    input_path: String,
    preview_duration: u32,
    options_key: String,
}

/// Cached transcoded preview entry.
struct CachedTranscodeEntry {
    output_path: PathBuf,
    estimated_size: u64,
}

/// LRU cache for transcoded preview outputs. Front = least recent, back = most recent.
struct TranscodeCache {
    entries: VecDeque<(TranscodeCacheKey, CachedTranscodeEntry)>,
}

impl TranscodeCache {
    fn new() -> Self {
        Self {
            entries: VecDeque::new(),
        }
    }

    /// Move entry at index to back (most recent) for LRU. Returns output path and size if found.
    fn move_entry_to_back(&mut self, idx: usize) -> Option<(PathBuf, u64)> {
        let (k, e) = self.entries.remove(idx)?;
        let result = (e.output_path.clone(), e.estimated_size);
        self.entries.push_back((k, e));
        Some(result)
    }
}

static TRANSCODE_CACHE: OnceLock<Mutex<TranscodeCache>> = OnceLock::new();

fn transcode_cache() -> &'static Mutex<TranscodeCache> {
    TRANSCODE_CACHE.get_or_init(|| Mutex::new(TranscodeCache::new()))
}

/// Cache for extracted preview segments: (input_path, preview_duration) -> segment paths.
/// Reused when only transcode options change, avoiding slow re-extraction from large files.
/// Stores 1 path for single-segment (short videos) or 3 paths for multi-segment (begin, mid, end).
struct CachedExtract {
    input_path: String,
    preview_duration: u32,
    segment_paths: Vec<PathBuf>,
}
static EXTRACT_CACHE: Mutex<Option<CachedExtract>> = Mutex::new(None);

/// Get cached segment paths if they match and all files exist.
pub fn get_cached_extract(input_path: &str, preview_duration: u32) -> Option<Vec<PathBuf>> {
    let guard = EXTRACT_CACHE.lock();
    guard.as_ref().and_then(|c| {
        if c.input_path == input_path
            && c.preview_duration == preview_duration
            && c.segment_paths.iter().all(|p| p.exists())
        {
            Some(c.segment_paths.clone())
        } else {
            None
        }
    })
}

/// Store or update the extract cache. Removes old cached segment files when replacing the cache entry.
pub fn set_cached_extract(input_path: String, preview_duration: u32, segment_paths: Vec<PathBuf>) {
    let mut guard = EXTRACT_CACHE.lock();
    if let Some(old) = guard.take() {
        for path in &old.segment_paths {
            log::trace!(
                target: "tiny_vid::ffmpeg::cache",
                "set_cached_extract: removing old cache {}",
                path.display()
            );
            let _ = fs::remove_file(path);
        }
    }
    log::debug!(
        target: "tiny_vid::ffmpeg::cache",
        "set_cached_extract: caching {} segments for input={}, duration={}",
        segment_paths.len(),
        input_path,
        preview_duration
    );
    *guard = Some(CachedExtract {
        input_path,
        preview_duration,
        segment_paths,
    });
}

/// Get cached transcoded preview if key matches and file exists.
/// On hit, moves entry to back (most recent) for LRU.
pub fn get_cached_preview_transcode(
    input_path: &str,
    preview_duration: u32,
    options: &TranscodeOptions,
) -> Option<(PathBuf, u64)> {
    let options_key = options.options_cache_key();
    let key = TranscodeCacheKey {
        input_path: input_path.to_string(),
        preview_duration,
        options_key: options_key.clone(),
    };
    let mut guard = transcode_cache().lock();
    let idx = guard.entries.iter().position(|(k, _)| k == &key);
    let Some(idx) = idx else {
        return None;
    };
    let (_, ref entry) = guard.entries[idx];
    if !entry.output_path.exists() {
        return None;
    }
    guard.move_entry_to_back(idx)
}

/// Returns all cached transcode output paths for the given input and duration.
/// Used by cleanup to preserve cached files.
pub fn get_cached_transcode_paths_to_keep(
    input_path: &str,
    preview_duration: u32,
) -> Vec<PathBuf> {
    let guard = transcode_cache().lock();
    guard
        .entries
        .iter()
        .filter(|(k, _)| k.input_path == input_path && k.preview_duration == preview_duration)
        .filter(|(_, e)| e.output_path.exists())
        .map(|(_, e)| e.output_path.clone())
        .collect()
}

/// Store transcoded preview in cache. Evicts LRU entries when over limit.
pub fn set_cached_preview_transcode(
    input_path: String,
    preview_duration: u32,
    options: &TranscodeOptions,
    output_path: PathBuf,
    estimated_size: u64,
) {
    let options_key = options.options_cache_key();
    let key = TranscodeCacheKey {
        input_path: input_path.clone(),
        preview_duration,
        options_key: options_key.clone(),
    };
    let mut guard = transcode_cache().lock();

    if let Some(idx) = guard.entries.iter().position(|(k, _)| k == &key) {
        let (_, old_entry) = guard.entries.remove(idx).unwrap();
        log::trace!(
            target: "tiny_vid::ffmpeg::cache",
            "set_cached_preview_transcode: replacing existing entry {}",
            old_entry.output_path.display()
        );
        let _ = fs::remove_file(&old_entry.output_path);
    }

    while guard.entries.len() >= PREVIEW_TRANSCODE_CACHE_MAX_ENTRIES {
        let Some((_, evicted)) = guard.entries.pop_front() else {
            break;
        };
        log::trace!(
            target: "tiny_vid::ffmpeg::cache",
            "set_cached_preview_transcode: evicting LRU {}",
            evicted.output_path.display()
        );
        let _ = fs::remove_file(&evicted.output_path);
    }

    log::debug!(
        target: "tiny_vid::ffmpeg::cache",
        "set_cached_preview_transcode: caching output for input={}, duration={}",
        input_path,
        preview_duration
    );
    guard.entries.push_back((
        key,
        CachedTranscodeEntry {
            output_path,
            estimated_size,
        },
    ));
}

/// Remove all cached transcode files and clear the cache. Call on app exit.
pub fn cleanup_preview_transcode_cache() {
    let mut guard = transcode_cache().lock();
    for (_, entry) in guard.entries.drain(..) {
        log::trace!(
            target: "tiny_vid::ffmpeg::cache",
            "cleanup_preview_transcode_cache: removing {}",
            entry.output_path.display()
        );
        let _ = fs::remove_file(&entry.output_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffmpeg::TempFileManager;

    #[test]
    fn lru_evicts_oldest_when_over_limit() {
        cleanup_preview_transcode_cache();

        let input = std::env::temp_dir().join("lru_test_input.mp4");
        let _ = fs::write(&input, b"fake");
        let input_str = input.to_string_lossy().to_string();

        let temp = TempFileManager::default();
        let mut first_path: Option<PathBuf> = None;
        for i in 0..PREVIEW_TRANSCODE_CACHE_MAX_ENTRIES + 1 {
            let path = temp
                .create(&format!("lru-test-{}.mp4", i), Some(b"x"))
                .unwrap();
            if i == 0 {
                first_path = Some(path.clone());
            }
            let mut opts = TranscodeOptions::default();
            opts.preset = Some(format!("preset_{}", i));
            set_cached_preview_transcode(
                input_str.clone(),
                3,
                &opts,
                path,
                1000,
            );
        }

        let p = first_path.unwrap();
        assert!(!p.exists(), "LRU should have evicted the first entry's file");
        cleanup_preview_transcode_cache();
        let _ = fs::remove_file(&input);
    }

    #[test]
    fn get_cached_transcode_paths_to_keep_returns_matching_paths() {
        cleanup_preview_transcode_cache();

        let input = std::env::temp_dir().join("paths_test_input.mp4");
        let _ = fs::write(&input, b"fake");
        let input_str = input.to_string_lossy().to_string();

        let temp = TempFileManager::default();
        let path1 = temp.create("paths-test-1.mp4", Some(b"a")).unwrap();
        let path2 = temp.create("paths-test-2.mp4", Some(b"b")).unwrap();

        let mut opts1 = TranscodeOptions::default();
        opts1.preset = Some("p1".into());
        let mut opts2 = TranscodeOptions::default();
        opts2.preset = Some("p2".into());

        set_cached_preview_transcode(input_str.clone(), 3, &opts1, path1.clone(), 100);
        set_cached_preview_transcode(input_str.clone(), 3, &opts2, path2.clone(), 200);

        let kept = get_cached_transcode_paths_to_keep(&input_str, 3);
        assert_eq!(kept.len(), 2);
        assert!(kept.contains(&path1));
        assert!(kept.contains(&path2));

        let kept_other_duration = get_cached_transcode_paths_to_keep(&input_str, 5);
        assert!(kept_other_duration.is_empty());

        cleanup_preview_transcode_cache();
        let _ = fs::remove_file(&input);
    }
}
