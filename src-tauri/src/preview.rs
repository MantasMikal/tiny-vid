//! Preview generation for video compression.

use std::fs;
use std::path::PathBuf;

use crate::error::AppError;
use crate::ffmpeg::{
    build_extract_args, build_ffmpeg_command, cleanup_previous_preview_paths,
    get_cached_preview, get_cached_segments, path_to_string, run_ffmpeg_blocking,
    set_cached_preview, store_preview_paths_for_cleanup,
    TempFileManager, TranscodeOptions,
};
use crate::ffmpeg::ffprobe::get_video_metadata_impl;
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
        Ok(Ok(())) => {
            log::trace!(
                target: "tiny_vid::preview",
                "emitting ffmpeg-complete to window={}",
                window_label_owned
            );
            let _ = app.emit_to(&window_label_owned, "ffmpeg-complete", ());
            Ok(())
        }
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

/// Extracts preview segments from input, or returns cached segment paths if available.
async fn extract_segments_or_use_cache(
    input_str: &str,
    preview_duration_u32: u32,
    segments: &[(f64, f64)],
    temp: &TempFileManager,
    emit: Option<(&tauri::AppHandle, &str)>,
) -> Result<Vec<PathBuf>, AppError> {
    match get_cached_segments(input_str, preview_duration_u32) {
        Some(cached) => {
            log::info!(
                target: "tiny_vid::preview",
                "extract_segments_or_use_cache: cache hit, reusing {} extracted segment(s)",
                cached.len()
            );
            Ok(cached)
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
            Ok(paths)
        }
    }
}

/// Transcodes a single segment and computes size estimate.
async fn transcode_single_segment(
    input_path: &PathBuf,
    segment_paths: &[PathBuf],
    output_path: &PathBuf,
    options: &TranscodeOptions,
    emit: Option<(&tauri::AppHandle, &str)>,
) -> Result<TranscodeResult, AppError> {
    let args = build_ffmpeg_command(
        &path_to_string(&segment_paths[0]),
        &path_to_string(output_path),
        options,
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
    emit: Option<(&tauri::AppHandle, &str)>,
) -> Result<TranscodeResult, AppError> {
    let ext = options.effective_output_format();
    let output_paths: Vec<PathBuf> = (0..segment_paths.len())
        .map(|i| {
            TempFileManager::default()
                .create(&format!("preview-compressed-{}.{}", i, ext), None)
                .map_err(AppError::from)
        })
        .collect::<Result<Vec<_>, _>>()?;

    for (orig, out) in segment_paths.iter().zip(output_paths.iter()) {
        let args =
            build_ffmpeg_command(&path_to_string(orig), &path_to_string(out), options)?;
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
    let mut ratio_sum = 0.0;
    for (orig, compressed) in segment_paths.iter().zip(output_paths.iter()) {
        let orig_size = fs::metadata(orig)?.len();
        let comp_size = fs::metadata(compressed)?.len();
        ratio_sum += comp_size as f64 / orig_size as f64;
    }
    let ratio_avg = ratio_sum / segment_paths.len() as f64;
    let estimated_size = (input_size as f64 * ratio_avg) as u64;

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
    emit: Option<(&tauri::AppHandle, &str)>,
) -> Result<TranscodeResult, AppError> {
    if is_multi_segment {
        transcode_multi_segment(input_path, segment_paths, output_path, options, emit).await
    } else {
        transcode_single_segment(input_path, segment_paths, output_path, options, emit).await
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

    log::info!(
        target: "tiny_vid::preview",
        "run_preview_core: input={}",
        input_path.display()
    );

    if let Some((original_path, compressed_path, estimated_size)) =
        get_cached_preview(&input_str, preview_duration_u32, &options)
    {
        log::info!(
            target: "tiny_vid::preview",
            "run_preview_core: cache hit, reusing output"
        );
        return Ok(PreviewResult {
            original_path: path_to_string(&original_path),
            compressed_path: path_to_string(&compressed_path),
            estimated_size,
        });
    }

    cleanup_previous_preview_paths(&input_str, preview_duration_u32);

    let meta = get_video_metadata_impl(&input_path)?;
    let video_duration = meta.duration;
    let segments = compute_preview_segments(video_duration, preview_duration);
    let is_multi_segment = segments.len() > 1;

    let ext = options.effective_output_format();
    let preview_suffix = format!("preview-output.{}", ext);

    let temp = TempFileManager::default();
    let output_path = temp
        .create(&preview_suffix, None)
        .map_err(AppError::from)?;

    let emit_ref = emit.as_ref().map(|(a, l)| (a, l.as_str()));

    let segment_paths = extract_segments_or_use_cache(
        &input_str,
        preview_duration_u32,
        &segments,
        &temp,
        emit_ref,
    )
    .await?;

    let transcode_result = transcode_segments_and_estimate(
        &input_path,
        &segment_paths,
        &output_path,
        &options,
        is_multi_segment,
        emit_ref,
    )
    .await?;

    store_preview_paths_for_cleanup(&segment_paths, &transcode_result.paths_for_cleanup);
    set_cached_preview(
        input_str.clone(),
        preview_duration_u32,
        &options,
        segment_paths.clone(),
        output_path.clone(),
        transcode_result.estimated_size,
    );
    log::info!(
        target: "tiny_vid::preview",
        "run_preview_core: complete, estimated_size={}",
        transcode_result.estimated_size
    );
    Ok(transcode_result.into())
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
