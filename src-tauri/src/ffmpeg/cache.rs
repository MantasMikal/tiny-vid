//! Unified preview cache: LRU 16 entries with segment reuse via ref-counting.
//!
//! Each preview result is (input, duration, preview_start_ms, options) -> output_path.
//! Segments are shared: (input, duration, preview_start_ms) -> (segment_paths, ref_count).
//! When evicting an LRU entry, we decrement segment ref_count; when it hits 0, we delete segment files.

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use parking_lot::Mutex;
use super::TranscodeOptions;

const PREVIEW_CACHE_MAX_ENTRIES: usize = 16;

#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub struct FileSignature {
    size: u64,
    modified_ms: u128,
}

pub fn file_signature(path: &Path) -> Option<FileSignature> {
    let meta = fs::metadata(path).ok()?;
    file_signature_from_metadata(&meta)
}

fn file_signature_from_metadata(meta: &fs::Metadata) -> Option<FileSignature> {
    let size = meta.len();
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis())?;
    Some(FileSignature {
        size,
        modified_ms: modified,
    })
}

/// Key for a full preview: (input_path, preview_duration, preview_start_ms, options_key, file_signature).
#[derive(Clone, Hash, Eq, PartialEq, Debug)]
struct PreviewCacheKey {
    input_path: String,
    preview_duration: u32,
    preview_start_ms: u64,
    options_key: String,
    file_signature: FileSignature,
}

/// Key for segment store: (input_path, preview_duration, preview_start_ms, file_signature).
#[derive(Clone, Hash, Eq, PartialEq, Debug)]
struct SegmentKey {
    input_path: String,
    preview_duration: u32,
    preview_start_ms: u64,
    file_signature: FileSignature,
}

/// Key for estimate cache: (input_path, preview_duration, options_key, file_signature).
#[derive(Clone, Hash, Eq, PartialEq, Debug)]
struct EstimateKey {
    input_path: String,
    preview_duration: u32,
    options_key: String,
    file_signature: FileSignature,
}

/// Segment store entry with ref count. Segments are shared across transcodes with same (input, duration).
struct SegmentEntry {
    segment_paths: Vec<PathBuf>,
    ref_count: u32,
}

/// LRU entry: output path and estimated size. Segment paths come from segment store.
struct PreviewEntry {
    output_path: PathBuf,
}

/// Unified preview cache: LRU for results, separate segment store with ref-counting.
struct PreviewCache {
    /// LRU: front = least recent, back = most recent.
    lru: VecDeque<(PreviewCacheKey, PreviewEntry)>,
    /// Segments keyed by (input, duration, preview_start_ms). Ref count = number of LRU entries using them.
    segments: HashMap<SegmentKey, SegmentEntry>,
    /// Estimated sizes keyed by (input, duration, options_key).
    estimates: HashMap<EstimateKey, u64>,
}

impl PreviewCache {
    fn new() -> Self {
        Self {
            lru: VecDeque::new(),
            segments: HashMap::new(),
            estimates: HashMap::new(),
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
            preview_start_ms: key.preview_start_ms,
            file_signature: key.file_signature,
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

    fn drop_preview_entry(&mut self, key: PreviewCacheKey, entry: PreviewEntry) {
        let _ = fs::remove_file(&entry.output_path);
        let seg_key = SegmentKey {
            input_path: key.input_path,
            preview_duration: key.preview_duration,
            preview_start_ms: key.preview_start_ms,
            file_signature: key.file_signature,
        };
        if let Some(seg) = self.segments.get_mut(&seg_key) {
            seg.ref_count = seg.ref_count.saturating_sub(1);
            if seg.ref_count == 0 {
                for path in &seg.segment_paths {
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

/// Get cached segments for (input, duration, preview_start_ms). Used to reuse extraction when only options change.
pub fn get_cached_segments(
    input_path: &str,
    preview_duration: u32,
    preview_start_ms: u64,
    file_signature: Option<&FileSignature>,
) -> Option<Vec<PathBuf>> {
    let file_signature = file_signature?.clone();
    let mut guard = preview_cache().lock();
    let key = SegmentKey {
        input_path: input_path.to_string(),
        preview_duration,
        preview_start_ms,
        file_signature,
    };
    let entry = guard.segments.get(&key)?;
    if entry.segment_paths.iter().all(|p| p.exists()) {
        Some(entry.segment_paths.clone())
    } else {
        guard.segments.remove(&key);
        None
    }
}

/// Get full cached preview. Returns (original_segment_path, compressed_path).
/// Both paths are always present together — no extract/transcode mismatch.
pub fn get_cached_preview(
    input_path: &str,
    preview_duration: u32,
    preview_start_ms: u64,
    options: &TranscodeOptions,
    file_signature: Option<&FileSignature>,
) -> Option<(PathBuf, PathBuf)> {
    let file_signature = file_signature?.clone();
    let options_key = options.options_cache_key_for_preview();
    let key = PreviewCacheKey {
        input_path: input_path.to_string(),
        preview_duration,
        preview_start_ms,
        options_key: options_key.clone(),
        file_signature,
    };

    let mut guard = preview_cache().lock();
    let idx = guard.lru.iter().position(|(k, _)| k == &key)?;
    let (k, entry) = guard.lru.remove(idx)?;
    if !entry.output_path.exists() {
        guard.drop_preview_entry(k, entry);
        return None;
    }

    let seg_key = SegmentKey {
        input_path: key.input_path,
        preview_duration: key.preview_duration,
        preview_start_ms: key.preview_start_ms,
        file_signature: key.file_signature,
    };
    let Some(seg_entry) = guard.segments.get(&seg_key) else {
        guard.drop_preview_entry(k, entry);
        return None;
    };
    if !seg_entry.segment_paths.iter().all(|p| p.exists()) {
        guard.drop_preview_entry(k, entry);
        return None;
    }
    let first_segment = seg_entry.segment_paths.first()?.clone();

    let result = (first_segment, entry.output_path.clone());
    guard.lru.push_back((k, entry));
    Some(result)
}

/// Get cached estimate for (input, duration, options).
pub fn get_cached_estimate(
    input_path: &str,
    preview_duration: u32,
    options: &TranscodeOptions,
    file_signature: Option<&FileSignature>,
) -> Option<u64> {
    let file_signature = file_signature?.clone();
    let options_key = options.options_cache_key_for_preview();
    let key = EstimateKey {
        input_path: input_path.to_string(),
        preview_duration,
        options_key,
        file_signature,
    };
    let guard = preview_cache().lock();
    guard.estimates.get(&key).copied()
}

/// Store cached estimate for (input, duration, options).
pub fn set_cached_estimate(
    input_path: &str,
    preview_duration: u32,
    options: &TranscodeOptions,
    estimated_size: u64,
    file_signature: Option<&FileSignature>,
) {
    let Some(file_signature) = file_signature.cloned() else {
        return;
    };
    let options_key = options.options_cache_key_for_preview();
    let key = EstimateKey {
        input_path: input_path.to_string(),
        preview_duration,
        options_key,
        file_signature,
    };
    let mut guard = preview_cache().lock();
    guard.estimates.insert(key, estimated_size);
}

/// Returns all cached paths (segments + outputs).
/// Used by cleanup to preserve cached files.
pub fn get_all_cached_paths() -> Vec<PathBuf> {
    let guard = preview_cache().lock();
    let mut paths = Vec::new();
    for seg in guard.segments.values() {
        paths.extend(seg.segment_paths.iter().filter(|p| p.exists()).cloned());
    }
    for (_k, e) in &guard.lru {
        if e.output_path.exists() {
            paths.push(e.output_path.clone());
        }
    }
    paths
}

/// Store preview in cache. Reuses segments if (input, duration) already exists.
pub fn set_cached_preview(
    input_path: &str,
    preview_duration: u32,
    preview_start_ms: u64,
    options: &TranscodeOptions,
    segment_paths: Vec<PathBuf>,
    output_path: PathBuf,
    file_signature: Option<&FileSignature>,
) {
    let Some(file_signature) = file_signature.cloned() else {
        return;
    };
    let input_path_owned = input_path.to_string();
    let options_key = options.options_cache_key_for_preview();
    let key = PreviewCacheKey {
        input_path: input_path_owned.clone(),
        preview_duration,
        preview_start_ms,
        options_key: options_key.clone(),
        file_signature: file_signature.clone(),
    };
    let seg_key = SegmentKey {
        input_path: input_path_owned,
        preview_duration,
        preview_start_ms,
        file_signature,
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
            preview_start_ms: old_key.preview_start_ms,
            file_signature: old_key.file_signature,
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
        "caching preview for input={}, duration={}, start_ms={}",
        input_path,
        preview_duration,
        preview_start_ms
    );
    guard.lru.push_back((
        key,
        PreviewEntry { output_path },
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
    guard.estimates.clear();
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
        let sig = file_signature(&input).unwrap();

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
                &input_str,
                3,
                0,
                &opts,
                vec![seg],
                out,
                Some(&sig),
            );
        }

        let p = first_output.unwrap();
        assert!(!p.exists(), "LRU should have evicted the first entry's output");
        cleanup_preview_transcode_cache();
        let _ = fs::remove_file(&input);
    }

    #[test]
    #[serial]
    fn get_all_cached_paths_returns_matching_paths() {
        cleanup_preview_transcode_cache();

        let input = std::env::temp_dir().join("paths_test_input.mp4");
        let _ = fs::write(&input, b"fake");
        let input_str = input.to_string_lossy().to_string();
        let sig = file_signature(&input).unwrap();

        let temp = TempFileManager::default();
        let seg = temp.create("paths-seg.mp4", Some(b"s")).unwrap();
        let path1 = temp.create("paths-out-1.mp4", Some(b"a")).unwrap();
        let path2 = temp.create("paths-out-2.mp4", Some(b"b")).unwrap();

        let mut opts1 = TranscodeOptions::default();
        opts1.preset = Some("p1".into());
        let mut opts2 = TranscodeOptions::default();
        opts2.preset = Some("p2".into());

        set_cached_preview(
            &input_str,
            3,
            0,
            &opts1,
            vec![seg.clone()],
            path1.clone(),
            Some(&sig),
        );
        set_cached_preview(
            &input_str,
            3,
            0,
            &opts2,
            vec![seg.clone()],
            path2.clone(),
            Some(&sig),
        );

        let kept = get_all_cached_paths();
        assert!(kept.contains(&seg));
        assert!(kept.contains(&path1));
        assert!(kept.contains(&path2));

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
        let sig = file_signature(&input).unwrap();

        let temp = TempFileManager::default();
        let seg = temp.create("preview-seg.mp4", Some(b"s")).unwrap();
        let out = temp.create("preview-out.mp4", Some(b"o")).unwrap();

        let opts = TranscodeOptions::default();
        set_cached_preview(
            &input_str,
            3,
            0,
            &opts,
            vec![seg.clone()],
            out.clone(),
            Some(&sig),
        );

        let result = get_cached_preview(&input_str, 3, 0, &opts, Some(&sig)).unwrap();
        assert_eq!(result.0, seg);
        assert_eq!(result.1, out);

        cleanup_preview_transcode_cache();
        let _ = fs::remove_file(&input);
    }

    #[test]
    #[serial]
    fn estimate_cache_round_trip() {
        cleanup_preview_transcode_cache();

        let input = std::env::temp_dir().join("estimate_cache_input.mp4");
        let _ = fs::write(&input, b"fake");
        let input_str = input.to_string_lossy().to_string();
        let sig = file_signature(&input).unwrap();

        let mut opts = TranscodeOptions::default();
        opts.preset = Some("fast".into());

        set_cached_estimate(&input_str, 3, &opts, 123, Some(&sig));
        let cached = get_cached_estimate(&input_str, 3, &opts, Some(&sig));
        assert_eq!(cached, Some(123));

        cleanup_preview_transcode_cache();
        let _ = fs::remove_file(&input);
    }

    #[test]
    #[serial]
    fn preview_cache_distinguishes_start_offsets() {
        cleanup_preview_transcode_cache();

        let input = std::env::temp_dir().join("preview_start_cache_input.mp4");
        let _ = fs::write(&input, b"fake");
        let input_str = input.to_string_lossy().to_string();
        let sig = file_signature(&input).unwrap();

        let temp = TempFileManager::default();
        let seg_a = temp.create("preview-start-a.mp4", Some(b"a")).unwrap();
        let out_a = temp.create("preview-out-a.mp4", Some(b"a")).unwrap();
        let seg_b = temp.create("preview-start-b.mp4", Some(b"b")).unwrap();
        let out_b = temp.create("preview-out-b.mp4", Some(b"b")).unwrap();

        let opts = TranscodeOptions::default();

        set_cached_preview(
            &input_str,
            3,
            0,
            &opts,
            vec![seg_a.clone()],
            out_a.clone(),
            Some(&sig),
        );
        set_cached_preview(
            &input_str,
            3,
            1000,
            &opts,
            vec![seg_b.clone()],
            out_b.clone(),
            Some(&sig),
        );

        let result_a = get_cached_preview(&input_str, 3, 0, &opts, Some(&sig)).unwrap();
        let result_b = get_cached_preview(&input_str, 3, 1000, &opts, Some(&sig)).unwrap();
        assert_eq!(result_a.1, out_a);
        assert_eq!(result_b.1, out_b);

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
        let sig = file_signature(&input).unwrap();

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
        set_cached_preview(
            &input_str,
            3,
            0,
            &opts1,
            vec![seg1.clone()],
            out1.clone(),
            Some(&sig),
        );

        // Second: same (input, duration), different segment paths (race scenario)
        set_cached_preview(
            &input_str,
            3,
            0,
            &opts2,
            vec![seg2.clone()],
            out2.clone(),
            Some(&sig),
        );

        // Fix: seg2 should be deleted (redundant extraction)
        assert!(!seg2.exists(), "redundant segment files should be deleted");
        assert!(seg1.exists(), "original segment files should remain");

        cleanup_preview_transcode_cache();
        let _ = fs::remove_file(&input);
    }
}
