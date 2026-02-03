//! Preview generation for video compression.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::ffmpeg::{
    build_extract_args, build_ffmpeg_command, cleanup_previous_preview_paths,
    file_signature, get_cached_preview, get_cached_segments, path_to_string, run_ffmpeg_blocking,
    set_cached_preview, store_preview_paths_for_cleanup,
    FileSignature, TempFileManager, TranscodeOptions,
};
use crate::ffmpeg::ffprobe::{get_video_metadata_impl, VideoMetadata};
use crate::ffmpeg::parse_ffmpeg_error;
use tauri::Emitter;

/// Optional emit context for progress events: (AppHandle, window label).
pub(crate) type PreviewEmit = Option<(tauri::AppHandle, String)>;

/// Runs FFmpeg with optional progress emission. pub(crate) for use by commands.
pub(crate) async fn run_ffmpeg_step(
    args: Vec<String>,
    app: &tauri::AppHandle,
    window_label: &str,
    duration_secs: Option<f64>,
) -> Result<(), AppError> {
    let app_for_blocking = app.clone();
    let window_label_owned = window_label.to_string();
    let result = tauri::async_runtime::spawn_blocking({
        let label = window_label_owned.clone();
        move || run_ffmpeg_blocking(args, Some(&app_for_blocking), Some(&label), duration_secs, None)
    })
    .await;

    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => {
            log::error!(target: "tiny_vid::preview", "ffmpeg-error: {}", e);
            let payload = match &e {
                AppError::FfmpegFailed { code, stderr } => parse_ffmpeg_error(stderr, Some(*code)),
                _ => parse_ffmpeg_error(&e.to_string(), None),
            };
            let _ = app.emit_to(&window_label_owned, "ffmpeg-error", payload);
            Err(e)
        }
        Err(join_err) => {
            let e = AppError::from(join_err.to_string());
            log::error!(target: "tiny_vid::preview", "ffmpeg-error (join): {}", e);
            let payload = parse_ffmpeg_error(&e.to_string(), None);
            let _ = app.emit_to(&window_label_owned, "ffmpeg-error", payload);
            Err(e)
        }
    }
}

struct TempCleanup {
    paths: Vec<PathBuf>,
    keep: bool,
}

impl TempCleanup {
    fn new() -> Self {
        Self {
            paths: Vec::new(),
            keep: false,
        }
    }

    fn add(&mut self, path: PathBuf) {
        self.paths.push(path);
    }

    fn keep(mut self) {
        self.keep = true;
    }
}

impl Drop for TempCleanup {
    fn drop(&mut self) {
        if self.keep {
            return;
        }
        for path in &self.paths {
            let _ = fs::remove_file(path);
        }
    }
}

struct SegmentSet {
    paths: Vec<PathBuf>,
    created: bool,
}

async fn get_video_metadata_async(path: &Path) -> Result<VideoMetadata, AppError> {
    let path = path.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || get_video_metadata_impl(&path))
        .await
        .map_err(|e| AppError::from(e.to_string()))?
}

/// Extracts preview segments from input, or returns cached segment paths if available.
async fn extract_segments_or_use_cache(
    input_str: &str,
    preview_duration_u32: u32,
    segments: &[(f64, f64)],
    temp: &TempFileManager,
    file_signature: Option<&FileSignature>,
    emit: Option<(&tauri::AppHandle, &str)>,
) -> Result<SegmentSet, AppError> {
    match get_cached_segments(input_str, preview_duration_u32, file_signature) {
        Some(cached) => {
            log::info!(
                target: "tiny_vid::preview",
                "extract_segments_or_use_cache: cache hit, reusing {} extracted segment(s)",
                cached.len()
            );
            Ok(SegmentSet {
                paths: cached,
                created: false,
            })
        }
        None => {
            let paths: Vec<PathBuf> = segments
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    temp.create(&format!("preview-original-{}.mp4", i), None)
                        .map_err(AppError::from)
                })
                .collect::<Result<Vec<_>, _>>()?;

            for ((start, dur), path) in segments.iter().zip(paths.iter()) {
                let args = build_extract_args(input_str, *start, *dur, &path_to_string(path));
                match emit {
                    Some((app, label)) => run_ffmpeg_step(args, app, label, None).await?,
                    None => {
                        tauri::async_runtime::spawn_blocking(move || {
                            run_ffmpeg_blocking(args, None, None, None, None)
                        })
                        .await
                        .map_err(|e| AppError::ffmpeg_failed(-1, e.to_string()))??;
                    }
                }
            }
            Ok(SegmentSet {
                paths,
                created: true,
            })
        }
    }
}

/// Transcodes a single segment and computes size estimate.
/// Uses original segment duration for output so compressed matches original (avoids sync drift).
async fn transcode_single_segment(
    input_path: &PathBuf,
    segment_paths: &[PathBuf],
    output_path: &PathBuf,
    options: &TranscodeOptions,
    _cleanup: &mut TempCleanup,
    emit: Option<(&tauri::AppHandle, &str)>,
) -> Result<TranscodeResult, AppError> {
    let output_duration = get_video_metadata_async(&segment_paths[0])
        .await
        .ok()
        .filter(|m| m.duration > 0.0)
        .map(|m| m.duration);
    let args = build_ffmpeg_command(
        &path_to_string(&segment_paths[0]),
        &path_to_string(output_path),
        options,
        output_duration,
    )?;

    match emit {
        Some((app, label)) => run_ffmpeg_step(args, app, label, None).await?,
        None => {
            tauri::async_runtime::spawn_blocking(move || {
                run_ffmpeg_blocking(args, None, None, None, None)
            })
            .await
            .map_err(|e| AppError::ffmpeg_failed(-1, e.to_string()))??;
        }
    }

    let input_size = fs::metadata(input_path)?.len();
    let compressed_size = fs::metadata(output_path)?.len();
    let original_size = fs::metadata(&segment_paths[0])?.len();
    let ratio = compressed_size as f64 / original_size as f64;
    let estimated_size = (input_size as f64 * ratio) as u64;
    let max_reasonable = input_size.saturating_mul(2);
    let estimated_size = estimated_size.min(max_reasonable);

    Ok(TranscodeResult {
        original_path: path_to_string(&segment_paths[0]),
        compressed_path: path_to_string(output_path),
        estimated_size,
        paths_for_cleanup: vec![output_path.clone()],
    })
}

/// Transcodes multiple segments (begin/mid/end) and computes size estimate from average ratio.
async fn transcode_multi_segment(
    input_path: &PathBuf,
    segment_paths: &[PathBuf],
    output_path: &PathBuf,
    options: &TranscodeOptions,
    cleanup: &mut TempCleanup,
    emit: Option<(&tauri::AppHandle, &str)>,
) -> Result<TranscodeResult, AppError> {
    let ext = options.effective_output_format();
    let output_paths: Vec<PathBuf> = (0..segment_paths.len())
        .map(|i| {
            TempFileManager
                .create(&format!("preview-compressed-{}.{}", i, ext), None)
                .map_err(AppError::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    for path in &output_paths {
        cleanup.add(path.clone());
    }

    let first_segment_duration = get_video_metadata_async(&segment_paths[0])
        .await
        .ok()
        .filter(|m| m.duration > 0.0)
        .map(|m| m.duration);

    for (i, (orig, out)) in segment_paths.iter().zip(output_paths.iter()).enumerate() {
        let output_duration = if i == 0 {
            first_segment_duration
        } else {
            None
        };
        let args = build_ffmpeg_command(
            &path_to_string(orig),
            &path_to_string(out),
            options,
            output_duration,
        )?;
        match emit {
            Some((app, label)) => run_ffmpeg_step(args, app, label, None).await?,
            None => {
                tauri::async_runtime::spawn_blocking(move || {
                    run_ffmpeg_blocking(args, None, None, None, None)
                })
                .await
                .map_err(|e| AppError::ffmpeg_failed(-1, e.to_string()))??;
            }
        }
    }

    let input_size = fs::metadata(input_path)?.len();
    let mut total_orig: u64 = 0;
    let mut total_comp: u64 = 0;
    for (orig, compressed) in segment_paths.iter().zip(output_paths.iter()) {
        let orig_size = fs::metadata(orig)?.len();
        let comp_size = fs::metadata(compressed)?.len();
        total_orig = total_orig.saturating_add(orig_size);
        total_comp = total_comp.saturating_add(comp_size);
    }
    let ratio = if total_orig > 0 {
        total_comp as f64 / total_orig as f64
    } else {
        0.0
    };
    let estimated_size = (input_size as f64 * ratio) as u64;
    let max_reasonable = input_size.saturating_mul(2);
    let estimated_size = estimated_size.min(max_reasonable);

    fs::copy(&output_paths[0], output_path)?;

    let mut paths_for_cleanup = output_paths.clone();
    paths_for_cleanup.push(output_path.clone());

    Ok(TranscodeResult {
        original_path: path_to_string(&segment_paths[0]),
        compressed_path: path_to_string(output_path),
        estimated_size,
        paths_for_cleanup,
    })
}

/// Transcodes segment(s) and computes size estimate. Multi-segment: samples begin/mid/end.
async fn transcode_segments_and_estimate(
    input_path: &PathBuf,
    segment_paths: &[PathBuf],
    output_path: &PathBuf,
    options: &TranscodeOptions,
    is_multi_segment: bool,
    cleanup: &mut TempCleanup,
    emit: Option<(&tauri::AppHandle, &str)>,
) -> Result<TranscodeResult, AppError> {
    if is_multi_segment {
        transcode_multi_segment(
            input_path,
            segment_paths,
            output_path,
            options,
            cleanup,
            emit,
        )
        .await
    } else {
        transcode_single_segment(
            input_path,
            segment_paths,
            output_path,
            options,
            cleanup,
            emit,
        )
        .await
    }
}

/// Segment position for multi-segment preview: (start_offset_secs, duration_secs).
/// First segment is full preview_duration for display (user expects first N seconds).
/// Other segments are ~1s each for estimation (beginning, middle, end ratios).
pub fn compute_preview_segments(
    video_duration: f64,
    preview_duration: f64,
) -> Vec<(f64, f64)> {
    if video_duration <= 0.0 || preview_duration <= 0.0 {
        return vec![(0.0, preview_duration.max(1.0))];
    }
    if video_duration <= preview_duration {
        return vec![(0.0, video_duration)];
    }
    let segment_duration = preview_duration / 3.0;
    let mid_start = (video_duration / 2.0) - (segment_duration / 2.0);
    let end_start = (video_duration - segment_duration).max(0.0);
    vec![
        (0.0, preview_duration),
        (mid_start.max(0.0), segment_duration),
        (end_start, segment_duration),
    ]
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreviewResult {
    pub(crate) original_path: String,
    pub(crate) compressed_path: String,
    pub(crate) estimated_size: u64,
    /// Start offset (seconds) of the original. Compressed typically has 0. Used to delay compressed playback for sync.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) start_offset_seconds: Option<f64>,
}

/// Result of transcode step: paths and size estimate for cleanup and caching.
struct TranscodeResult {
    original_path: String,
    compressed_path: String,
    estimated_size: u64,
    paths_for_cleanup: Vec<PathBuf>,
}

impl From<TranscodeResult> for PreviewResult {
    fn from(t: TranscodeResult) -> Self {
        Self {
            original_path: t.original_path,
            compressed_path: t.compressed_path,
            estimated_size: t.estimated_size,
            start_offset_seconds: None,
        }
    }
}

/// Core preview logic. When emit is Some, emits progress events; when None, runs silently (for tests).
pub(crate) async fn run_preview_core(
    input_path: PathBuf,
    options: TranscodeOptions,
    emit: PreviewEmit,
) -> Result<PreviewResult, AppError> {
    let input_str = path_to_string(&input_path);
    let preview_duration_u32 = options.effective_preview_duration();
    let preview_duration = preview_duration_u32 as f64;
    let file_sig = file_signature(&input_path);

    log::info!(
        target: "tiny_vid::preview",
        "run_preview_core: input={}",
        input_path.display()
    );

    if let Some((original_path, compressed_path, estimated_size)) =
        get_cached_preview(&input_str, preview_duration_u32, &options, file_sig.as_ref())
    {
        log::info!(
            target: "tiny_vid::preview",
            "run_preview_core: cache hit, reusing output"
        );
        let start_offset_seconds = get_video_metadata_async(&original_path)
            .await
            .ok()
            .and_then(|m| m.start_time);
        if let Some((app, label)) = emit.as_ref() {
            let _ = app.emit_to(label, "ffmpeg-complete", ());
        }
        return Ok(PreviewResult {
            original_path: path_to_string(&original_path),
            compressed_path: path_to_string(&compressed_path),
            estimated_size,
            start_offset_seconds,
        });
    }

    cleanup_previous_preview_paths(&input_str, preview_duration_u32);

    let meta = get_video_metadata_async(&input_path).await?;
    let video_duration = meta.duration;
    let segments = compute_preview_segments(video_duration, preview_duration);
    let is_multi_segment = segments.len() > 1;

    let ext = options.effective_output_format();
    let preview_suffix = format!("preview-output.{}", ext);

    let temp = TempFileManager;
    let output_path = temp
        .create(&preview_suffix, None)
        .map_err(AppError::from)?;
    let mut cleanup = TempCleanup::new();
    cleanup.add(output_path.clone());

    let emit_ref = emit.as_ref().map(|(a, l)| (a, l.as_str()));

    let segment_set = extract_segments_or_use_cache(
        &input_str,
        preview_duration_u32,
        &segments,
        &temp,
        file_sig.as_ref(),
        emit_ref,
    )
    .await?;
    if segment_set.created {
        for path in &segment_set.paths {
            cleanup.add(path.clone());
        }
    }

    let transcode_result = transcode_segments_and_estimate(
        &input_path,
        &segment_set.paths,
        &output_path,
        &options,
        is_multi_segment,
        &mut cleanup,
        emit_ref,
    )
    .await?;

    store_preview_paths_for_cleanup(&segment_set.paths, &transcode_result.paths_for_cleanup);
    set_cached_preview(
        input_str.clone(),
        preview_duration_u32,
        &options,
        segment_set.paths.clone(),
        output_path.clone(),
        transcode_result.estimated_size,
        file_sig.as_ref(),
    );
    let start_offset_seconds = get_video_metadata_async(&segment_set.paths[0])
        .await
        .ok()
        .and_then(|m| m.start_time);
    log::info!(
        target: "tiny_vid::preview",
        "run_preview_core: complete, estimated_size={}, start_offset_seconds={:?}",
        transcode_result.estimated_size,
        start_offset_seconds
    );
    if let Some((app, label)) = emit.as_ref() {
        let _ = app.emit_to(label, "ffmpeg-complete", ());
    }
    cleanup.keep();
    Ok(PreviewResult {
        original_path: transcode_result.original_path,
        compressed_path: transcode_result.compressed_path,
        estimated_size: transcode_result.estimated_size,
        start_offset_seconds,
    })
}

#[cfg(test)]
mod tests {
    use super::compute_preview_segments;

    #[test]
    fn single_segment_when_video_shorter_than_preview() {
        let segs = compute_preview_segments(2.0, 3.0);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0], (0.0, 2.0));
    }

    #[test]
    fn three_segments_when_video_longer_than_preview() {
        let segs = compute_preview_segments(60.0, 3.0);
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], (0.0, 3.0), "first segment is full preview for display");
        assert_eq!(segs[1], (29.5, 1.0));
        assert_eq!(segs[2], (59.0, 1.0));
    }

    #[test]
    fn single_segment_when_duration_equals_preview() {
        let segs = compute_preview_segments(3.0, 3.0);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0], (0.0, 3.0));
    }

    #[test]
    fn handles_zero_duration() {
        let segs = compute_preview_segments(0.0, 3.0);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].1, 3.0);
    }

    #[test]
    fn segment_positions_dont_overlap_for_short_video() {
        let segs = compute_preview_segments(5.0, 6.0);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0], (0.0, 5.0));
    }

    #[test]
    fn mid_segment_never_negative() {
        let segs = compute_preview_segments(4.0, 3.0);
        assert_eq!(segs.len(), 3);
        for (start, _dur) in &segs {
            assert!(
                *start >= 0.0,
                "segment start should never be negative: {}",
                start
            );
        }
    }
}
