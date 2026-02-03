//! Unified preview cache: LRU 16 entries with segment reuse via ref-counting.
//!
//! Each preview result is (input, duration, options) -> (output_path, estimated_size).
//! Segments are shared: (input, duration) -> (segment_paths, ref_count).
//! When evicting an LRU entry, we decrement segment ref_count; when it hits 0, we delete segment files.

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use parking_lot::Mutex;
use super::TranscodeOptions;

const PREVIEW_CACHE_MAX_ENTRIES: usize = 16;

/// Key for a full preview: (input_path, preview_duration, options_key).
#[derive(Clone, Hash, Eq, PartialEq, Debug)]
struct PreviewCacheKey {
    input_path: String,
    preview_duration: u32,
    options_key: String,
}

/// Key for segment store: (input_path, preview_duration).
#[derive(Clone, Hash, Eq, PartialEq, Debug)]
struct SegmentKey {
    input_path: String,
    preview_duration: u32,
}

/// Segment store entry with ref count. Segments are shared across transcodes with same (input, duration).
struct SegmentEntry {
    segment_paths: Vec<PathBuf>,
    ref_count: u32,
}

/// LRU entry: output path and estimated size. Segment paths come from segment store.
struct PreviewEntry {
    output_path: PathBuf,
    estimated_size: u64,
}

/// Unified preview cache: LRU for results, separate segment store with ref-counting.
struct PreviewCache {
    /// LRU: front = least recent, back = most recent.
    lru: VecDeque<(PreviewCacheKey, PreviewEntry)>,
    /// Segments keyed by (input, duration). Ref count = number of LRU entries using them.
    segments: HashMap<SegmentKey, SegmentEntry>,
}

impl PreviewCache {
    fn new() -> Self {
        Self {
            lru: VecDeque::new(),
            segments: HashMap::new(),
        }
    }

    fn evict_one(&mut self) {
        let Some((key, entry)) = self.lru.pop_front() else {
            return;
        };
        log::trace!(
            target: "tiny_vid::ffmpeg::cache",
            "evicting LRU entry output={}",
            entry.output_path.display()
        );
        let _ = fs::remove_file(&entry.output_path);

        let seg_key = SegmentKey {
            input_path: key.input_path,
            preview_duration: key.preview_duration,
        };
        if let Some(seg) = self.segments.get_mut(&seg_key) {
            seg.ref_count = seg.ref_count.saturating_sub(1);
            if seg.ref_count == 0 {
                for path in &seg.segment_paths {
                    log::trace!(
                        target: "tiny_vid::ffmpeg::cache",
                        "evicting segment {}",
                        path.display()
                    );
                    let _ = fs::remove_file(path);
                }
                self.segments.remove(&seg_key);
            }
        }
    }
}

static PREVIEW_CACHE: OnceLock<Mutex<PreviewCache>> = OnceLock::new();

fn preview_cache() -> &'static Mutex<PreviewCache> {
    PREVIEW_CACHE.get_or_init(|| Mutex::new(PreviewCache::new()))
}

/// Get cached segments for (input, duration). Used to reuse extraction when only options change.
pub fn get_cached_segments(input_path: &str, preview_duration: u32) -> Option<Vec<PathBuf>> {
    let guard = preview_cache().lock();
    let key = SegmentKey {
        input_path: input_path.to_string(),
        preview_duration,
    };
    guard.segments.get(&key).and_then(|e| {
        if e.segment_paths.iter().all(|p| p.exists()) {
            Some(e.segment_paths.clone())
        } else {
            None
        }
    })
}

/// Get full cached preview. Returns (original_segment_path, compressed_path, estimated_size).
/// Both paths are always present together — no extract/transcode mismatch.
pub fn get_cached_preview(
    input_path: &str,
    preview_duration: u32,
    options: &TranscodeOptions,
) -> Option<(PathBuf, PathBuf, u64)> {
    let options_key = options.options_cache_key();
    let key = PreviewCacheKey {
        input_path: input_path.to_string(),
        preview_duration,
        options_key: options_key.clone(),
    };

    let mut guard = preview_cache().lock();
    let idx = guard.lru.iter().position(|(k, _)| k == &key)?;
    let (k, entry) = guard.lru.remove(idx)?;
    if !entry.output_path.exists() {
        return None;
    }

    let seg_key = SegmentKey {
        input_path: key.input_path,
        preview_duration: key.preview_duration,
    };
    let first_segment = guard.segments.get(&seg_key)?.segment_paths.first()?.clone();
    if !first_segment.exists() {
        return None;
    }

    let result = (first_segment, entry.output_path.clone(), entry.estimated_size);
    guard.lru.push_back((k, entry));
    Some(result)
}

/// Returns all cached paths (segments + outputs) for the given input and duration.
/// Used by cleanup to preserve cached files.
pub fn get_cached_paths_to_keep(input_path: &str, preview_duration: u32) -> Vec<PathBuf> {
    let guard = preview_cache().lock();
    let seg_key = SegmentKey {
        input_path: input_path.to_string(),
        preview_duration,
    };
    let mut paths = Vec::new();
    if let Some(seg) = guard.segments.get(&seg_key) {
        paths.extend(seg.segment_paths.iter().filter(|p| p.exists()).cloned());
    }
    for (k, e) in &guard.lru {
        if k.input_path == input_path && k.preview_duration == preview_duration && e.output_path.exists() {
            paths.push(e.output_path.clone());
        }
    }
    paths
}

/// Store preview in cache. Reuses segments if (input, duration) already exists.
pub fn set_cached_preview(
    input_path: String,
    preview_duration: u32,
    options: &TranscodeOptions,
    segment_paths: Vec<PathBuf>,
    output_path: PathBuf,
    estimated_size: u64,
) {
    let options_key = options.options_cache_key();
    let key = PreviewCacheKey {
        input_path: input_path.clone(),
        preview_duration,
        options_key: options_key.clone(),
    };
    let seg_key = SegmentKey {
        input_path: input_path.clone(),
        preview_duration,
    };

    let mut guard = preview_cache().lock();

    if let Some(idx) = guard.lru.iter().position(|(k, _)| k == &key) {
        let (old_key, old_entry) = guard.lru.remove(idx).unwrap();
        log::trace!(
            target: "tiny_vid::ffmpeg::cache",
            "replacing existing entry {}",
            old_entry.output_path.display()
        );
        let _ = fs::remove_file(&old_entry.output_path);
        let old_seg_key = SegmentKey {
            input_path: old_key.input_path,
            preview_duration: old_key.preview_duration,
        };
        if let Some(seg) = guard.segments.get_mut(&old_seg_key) {
            seg.ref_count = seg.ref_count.saturating_sub(1);
            if seg.ref_count == 0 {
                let paths = seg.segment_paths.clone();
                guard.segments.remove(&old_seg_key);
                for path in paths {
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }

    while guard.lru.len() >= PREVIEW_CACHE_MAX_ENTRIES {
        guard.evict_one();
    }

    if let Some(seg) = guard.segments.get_mut(&seg_key) {
        seg.ref_count += 1;
        // Incoming paths are from a redundant extraction (race). Delete to avoid orphan.
        if segment_paths != seg.segment_paths {
            for path in &segment_paths {
                log::trace!(
                    target: "tiny_vid::ffmpeg::cache",
                    "deleting redundant segment {}",
                    path.display()
                );
                let _ = fs::remove_file(path);
            }
        }
    } else {
        guard.segments.insert(
            seg_key,
            SegmentEntry {
                segment_paths: segment_paths.clone(),
                ref_count: 1,
            },
        );
    }

    log::debug!(
        target: "tiny_vid::ffmpeg::cache",
        "caching preview for input={}, duration={}",
        input_path,
        preview_duration
    );
    guard.lru.push_back((
        key,
        PreviewEntry {
            output_path,
            estimated_size,
        },
    ));
}

/// Remove all cached files and clear the cache. Call on app exit.
pub fn cleanup_preview_transcode_cache() {
    let mut guard = preview_cache().lock();
    for (_, entry) in guard.lru.drain(..) {
        log::trace!(
            target: "tiny_vid::ffmpeg::cache",
            "cleanup removing {}",
            entry.output_path.display()
        );
        let _ = fs::remove_file(&entry.output_path);
    }
    for (_, seg) in guard.segments.drain() {
        for path in seg.segment_paths {
            log::trace!(
                target: "tiny_vid::ffmpeg::cache",
                "cleanup removing segment {}",
                path.display()
            );
            let _ = fs::remove_file(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffmpeg::TempFileManager;
    use serial_test::serial;

    #[test]
    #[serial]
    fn lru_evicts_oldest_when_over_limit() {
        cleanup_preview_transcode_cache();

        let input = std::env::temp_dir().join("lru_test_input.mp4");
        let _ = fs::write(&input, b"fake");
        let input_str = input.to_string_lossy().to_string();

        let temp = TempFileManager::default();
        let mut first_output: Option<PathBuf> = None;
        for i in 0..PREVIEW_CACHE_MAX_ENTRIES + 1 {
            let seg = temp.create(&format!("lru-seg-{}.mp4", i), Some(b"s")).unwrap();
            let out = temp.create(&format!("lru-out-{}.mp4", i), Some(b"x")).unwrap();
            if i == 0 {
                first_output = Some(out.clone());
            }
            let mut opts = TranscodeOptions::default();
            opts.preset = Some(format!("preset_{}", i));
            set_cached_preview(
                input_str.clone(),
                3,
                &opts,
                vec![seg],
                out,
                1000,
            );
        }

        let p = first_output.unwrap();
        assert!(!p.exists(), "LRU should have evicted the first entry's output");
        cleanup_preview_transcode_cache();
        let _ = fs::remove_file(&input);
    }

    #[test]
    #[serial]
    fn get_cached_paths_to_keep_returns_matching_paths() {
        cleanup_preview_transcode_cache();

        let input = std::env::temp_dir().join("paths_test_input.mp4");
        let _ = fs::write(&input, b"fake");
        let input_str = input.to_string_lossy().to_string();

        let temp = TempFileManager::default();
        let seg = temp.create("paths-seg.mp4", Some(b"s")).unwrap();
        let path1 = temp.create("paths-out-1.mp4", Some(b"a")).unwrap();
        let path2 = temp.create("paths-out-2.mp4", Some(b"b")).unwrap();

        let mut opts1 = TranscodeOptions::default();
        opts1.preset = Some("p1".into());
        let mut opts2 = TranscodeOptions::default();
        opts2.preset = Some("p2".into());

        set_cached_preview(input_str.clone(), 3, &opts1, vec![seg.clone()], path1.clone(), 100);
        set_cached_preview(input_str.clone(), 3, &opts2, vec![seg.clone()], path2.clone(), 200);

        let kept = get_cached_paths_to_keep(&input_str, 3);
        assert!(kept.contains(&seg));
        assert!(kept.contains(&path1));
        assert!(kept.contains(&path2));

        let kept_other = get_cached_paths_to_keep(&input_str, 5);
        assert!(kept_other.is_empty());

        cleanup_preview_transcode_cache();
        let _ = fs::remove_file(&input);
    }

    #[test]
    #[serial]
    fn get_cached_preview_returns_both_paths() {
        cleanup_preview_transcode_cache();

        let input = std::env::temp_dir().join("preview_test_input.mp4");
        let _ = fs::write(&input, b"fake");
        let input_str = input.to_string_lossy().to_string();

        let temp = TempFileManager::default();
        let seg = temp.create("preview-seg.mp4", Some(b"s")).unwrap();
        let out = temp.create("preview-out.mp4", Some(b"o")).unwrap();

        let opts = TranscodeOptions::default();
        set_cached_preview(
            input_str.clone(),
            3,
            &opts,
            vec![seg.clone()],
            out.clone(),
            500,
        );

        let result = get_cached_preview(&input_str, 3, &opts).unwrap();
        assert_eq!(result.0, seg);
        assert_eq!(result.1, out);
        assert_eq!(result.2, 500);

        cleanup_preview_transcode_cache();
        let _ = fs::remove_file(&input);
    }

    #[test]
    #[serial]
    fn set_cached_preview_deletes_redundant_segments_when_reusing() {
        cleanup_preview_transcode_cache();

        let input = std::env::temp_dir().join("redundant_seg_test_input.mp4");
        let _ = fs::write(&input, b"fake");
        let input_str = input.to_string_lossy().to_string();

        let temp = TempFileManager::default();
        let seg1 = temp.create("redundant-seg-1.mp4", Some(b"s1")).unwrap();
        let seg2 = temp.create("redundant-seg-2.mp4", Some(b"s2")).unwrap();
        let out1 = temp.create("redundant-out-1.mp4", Some(b"o1")).unwrap();
        let out2 = temp.create("redundant-out-2.mp4", Some(b"o2")).unwrap();

        let mut opts1 = TranscodeOptions::default();
        opts1.preset = Some("p1".into());
        let mut opts2 = TranscodeOptions::default();
        opts2.preset = Some("p2".into());

        // First: store seg1
        set_cached_preview(input_str.clone(), 3, &opts1, vec![seg1.clone()], out1.clone(), 100);

        // Second: same (input, duration), different segment paths (race scenario)
        set_cached_preview(input_str.clone(), 3, &opts2, vec![seg2.clone()], out2.clone(), 200);

        // Fix: seg2 should be deleted (redundant extraction)
        assert!(!seg2.exists(), "redundant segment files should be deleted");
        assert!(seg1.exists(), "original segment files should remain");

        cleanup_preview_transcode_cache();
        let _ = fs::remove_file(&input);
    }
}
