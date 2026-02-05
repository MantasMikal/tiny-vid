//! Preview generation for video compression.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::error::AppError;
use crate::ffmpeg::{
    build_extract_args, build_ffmpeg_command, cleanup_previous_preview_paths,
    file_signature, get_cached_estimate, get_cached_preview, get_cached_segments,
    is_browser_playable_codec, path_to_string, run_ffmpeg_blocking, set_cached_estimate,
    set_cached_preview, store_preview_paths_for_cleanup, FfmpegProgressPayload, FileSignature,
    TempFileManager, TranscodeOptions,
};
use crate::ffmpeg::ffprobe::{get_video_metadata_impl, VideoMetadata};
use crate::ffmpeg::parse_ffmpeg_error;
use tauri::Emitter;

/// Optional emit context for progress events: (AppHandle, window label).
pub(crate) type PreviewEmit = Option<(tauri::AppHandle, String)>;

/// Step counts for progress emission. Preview: extract + transcode. Estimate: N segments × (extract + transcode).
const PREVIEW_STEPS: usize = 2;

fn estimate_step_count(segment_count: usize) -> usize {
    segment_count * 2
}

/// Context for aggregating preview progress across multiple FFmpeg steps.
/// Supports sub-range emission via base_step for unified multi-phase progress.
/// Emits (base_step + step_index + p) / total_steps.
pub(crate) struct PreviewProgressCtx {
    app: tauri::AppHandle,
    label: String,
    step_index: AtomicUsize,
    base_step: usize,
    total_steps: usize,
}

impl PreviewProgressCtx {
    /// Create a progress context. Use base_step=0 for a standalone pipeline, or base_step>0 for a sub-range of a larger progress.
    fn new(
        app: tauri::AppHandle,
        label: String,
        base_step: usize,
        total_steps: usize,
    ) -> Self {
        Self {
            app,
            label,
            step_index: AtomicUsize::new(0),
            base_step,
            total_steps,
        }
    }

    fn make_callback(&self, step: &'static str) -> Arc<dyn Fn(f64) + Send + Sync> {
        let idx = self.step_index.load(Ordering::Relaxed);
        let app = self.app.clone();
        let label = self.label.clone();
        let base = self.base_step as f64;
        let total = self.total_steps as f64;
        let step_owned = step.to_string();
        Arc::new(move |p: f64| {
            let overall = (base + idx as f64 + p) / total;
            let payload = FfmpegProgressPayload {
                progress: overall,
                step: Some(step_owned.clone()),
            };
            let _ = app.emit_to(&label, "ffmpeg-progress", payload);
        })
    }

    fn advance(&self) {
        self.step_index.fetch_add(1, Ordering::Relaxed);
    }
}

/// Creates a callback that emits ffmpeg-progress with a step label.
/// Use for single-step operations (e.g. transcode).
pub(crate) fn make_progress_emitter(
    app: tauri::AppHandle,
    label: String,
    step: &'static str,
) -> Arc<dyn Fn(f64) + Send + Sync> {
    let step_owned = step.to_string();
    Arc::new(move |p: f64| {
        let payload = FfmpegProgressPayload {
            progress: p,
            step: Some(step_owned.clone()),
        };
        let _ = app.emit_to(&label, "ffmpeg-progress", payload);
    })
}

/// Runs FFmpeg with optional progress and error emission. pub(crate) for use by commands.
///
/// - `emit`: When Some, used for ffmpeg-error emission on failure. When `progress_callback` is
///   None, also passed to the runner for direct ffmpeg-progress emission.
/// - `progress_callback`: When Some, used for progress instead of direct emit (e.g. preview
///   aggregate progress). `emit` is still used for error emission.
pub(crate) async fn run_ffmpeg_step(
    args: Vec<String>,
    emit: Option<(&tauri::AppHandle, &str)>,
    duration_secs: Option<f64>,
    progress_callback: Option<std::sync::Arc<dyn Fn(f64) + Send + Sync>>,
) -> Result<(), AppError> {
    let (app_opt, label_opt) = emit
        .map(|(a, l)| (Some(a.clone()), Some(l.to_string())))
        .unwrap_or((None, None));
    let result = tauri::async_runtime::spawn_blocking({
        let app_for_blocking = app_opt.clone();
        let label_for_blocking = label_opt.clone();
        move || {
            run_ffmpeg_blocking(
                args,
                app_for_blocking.as_ref(),
                label_for_blocking.as_deref(),
                duration_secs,
                progress_callback,
                None,
            )
        }
    })
    .await;

    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => {
            log::error!(target: "tiny_vid::preview", "ffmpeg-error: {}", e);
            if let (Some(app), Some(label)) = (app_opt.as_ref(), label_opt.as_ref()) {
                let payload = match &e {
                    AppError::FfmpegFailed { code, stderr } => {
                        parse_ffmpeg_error(stderr, Some(*code))
                    }
                    _ => parse_ffmpeg_error(&e.to_string(), None),
                };
                let _ = app.emit_to(label, "ffmpeg-error", payload);
            }
            Err(e)
        }
        Err(join_err) => {
            let e = AppError::from(join_err.to_string());
            log::error!(target: "tiny_vid::preview", "ffmpeg-error (join): {}", e);
            if let (Some(app), Some(label)) = (app_opt.as_ref(), label_opt.as_ref()) {
                let payload = parse_ffmpeg_error(&e.to_string(), None);
                let _ = app.emit_to(label, "ffmpeg-error", payload);
            }
            Err(e)
        }
    }
}

/// Handles both emit (with app) and no-emit (silent) paths.
async fn run_ffmpeg_with_progress(
    args: Vec<String>,
    duration_secs: Option<f64>,
    emit: Option<(&tauri::AppHandle, &str)>,
    progress_ctx: Option<&PreviewProgressCtx>,
    step_label: &'static str,
) -> Result<(), AppError> {
    let progress_cb = progress_ctx.map(|ctx| ctx.make_callback(step_label));
    run_ffmpeg_step(args, emit, duration_secs, progress_cb).await?;
    if let Some(ctx) = progress_ctx {
        ctx.advance();
    }
    Ok(())
}

fn clamp_preview_start_seconds(
    requested: f64,
    video_duration: f64,
    preview_duration: f64,
) -> f64 {
    if !requested.is_finite() {
        return 0.0;
    }
    if video_duration <= 0.0 || preview_duration <= 0.0 {
        return 0.0;
    }
    let max_start = (video_duration - preview_duration).max(0.0);
    requested.max(0.0).min(max_start)
}

fn preview_start_ms_from_seconds(start_seconds: f64) -> u64 {
    if !start_seconds.is_finite() {
        return 0;
    }
    (start_seconds.max(0.0) * 1000.0).round() as u64
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
/// step_label: when progress_ctx is Some, label for progress ("extract" or "estimate").
async fn extract_segments_or_use_cache(
    input_str: &str,
    preview_duration_u32: u32,
    preview_start_ms: u64,
    segments: &[(f64, f64)],
    temp: &TempFileManager,
    file_signature: Option<&FileSignature>,
    emit: Option<(&tauri::AppHandle, &str)>,
    progress_ctx: Option<&PreviewProgressCtx>,
    step_label: &'static str,
) -> Result<SegmentSet, AppError> {
    match get_cached_segments(
        input_str,
        preview_duration_u32,
        preview_start_ms,
        file_signature,
    ) {
        Some(cached) => {
            log::info!(
                target: "tiny_vid::preview",
                "extract_segments_or_use_cache: cache hit, reusing {} extracted segment(s)",
                cached.len()
            );
            if let Some(ctx) = progress_ctx {
                for _ in segments {
                    let cb = ctx.make_callback(step_label);
                    cb(1.0);
                    ctx.advance();
                }
            }
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
                run_ffmpeg_with_progress(
                    args,
                    Some(*dur),
                    emit,
                    progress_ctx,
                    step_label,
                )
                .await?;
            }
            Ok(SegmentSet {
                paths,
                created: true,
            })
        }
    }
}

async fn transcode_preview_segment(
    segment_path: &PathBuf,
    output_path: &PathBuf,
    options: &TranscodeOptions,
    output_duration: Option<f64>,
    emit: Option<(&tauri::AppHandle, &str)>,
    progress_ctx: Option<&PreviewProgressCtx>,
) -> Result<(), AppError> {
    let args = build_ffmpeg_command(
        &path_to_string(segment_path),
        &path_to_string(output_path),
        options,
        output_duration,
        Some("mp4"),
        None,
    )?;

    run_ffmpeg_with_progress(
        args,
        output_duration,
        emit,
        progress_ctx,
        "preview_transcode",
    )
    .await?;
    Ok(())
}


/// Transcodes estimation segments (begin/mid/end) and computes size estimate.
/// Uses provided segment durations to avoid ffprobe calls on the extracted samples.
async fn estimate_size_from_segments(
    input_path: &Path,
    segment_paths: &[PathBuf],
    segment_durations: &[f64],
    options: &TranscodeOptions,
    cleanup: &mut TempCleanup,
    emit: Option<(&tauri::AppHandle, &str)>,
    progress_ctx: Option<&PreviewProgressCtx>,
) -> Result<u64, AppError> {
    if segment_paths.is_empty() {
        return Ok(0);
    }
    let output_paths: Vec<PathBuf> = (0..segment_paths.len())
        .map(|i| {
            TempFileManager
                .create(&format!("preview-estimate-{}.mp4", i), None)
                .map_err(AppError::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    for path in &output_paths {
        cleanup.add(path.clone());
    }

    for (i, (orig, out)) in segment_paths.iter().zip(output_paths.iter()).enumerate() {
        let output_duration = segment_durations
            .get(i)
            .copied()
            .filter(|d| *d > 0.0);
        let args = build_ffmpeg_command(
            &path_to_string(orig),
            &path_to_string(out),
            options,
            output_duration,
            Some("mp4"),
            None,
        )?;
        run_ffmpeg_with_progress(
            args,
            output_duration,
            emit,
            progress_ctx,
            "preview_estimate",
        )
        .await?;
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
    Ok(estimated_size.min(max_reasonable))
}

async fn compute_estimate_size(
    input_path: &Path,
    input_str: &str,
    preview_duration_u32: u32,
    preview_duration: f64,
    video_duration: f64,
    options: &TranscodeOptions,
    emit: Option<(&tauri::AppHandle, &str)>,
    progress_ctx: Option<&PreviewProgressCtx>,
) -> Result<u64, AppError> {
    let segments = compute_preview_segments(video_duration, preview_duration);
    let segment_durations: Vec<f64> = segments.iter().map(|(_, dur)| *dur).collect();
    let temp = TempFileManager;
    let mut cleanup = TempCleanup::new();
    // file_signature: None — estimate segments (begin/mid/end) are ephemeral; we transcode and discard.
    // We don't use the segment cache here to avoid key overlap with preview segments (which use
    // user-selected start) and to keep the segment store focused on preview reuse.
    let segment_set = extract_segments_or_use_cache(
        input_str,
        preview_duration_u32,
        0,
        &segments,
        &temp,
        None,
        emit,
        progress_ctx,
        "preview_estimate",
    )
    .await?;
    for path in &segment_set.paths {
        cleanup.add(path.clone());
    }
    estimate_size_from_segments(
        input_path,
        &segment_set.paths,
        &segment_durations,
        options,
        &mut cleanup,
        emit,
        progress_ctx,
    )
    .await
}

/// Segment positions for estimation: (start_offset_secs, duration_secs).
/// Uses begin/mid/end sampling; when video is shorter, returns a single segment.
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
    /// Start offset (seconds) of the original. Compressed typically has 0. Used to delay compressed playback for sync.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) start_offset_seconds: Option<f64>,
}

/// Result of preview with optional size estimate. Used when include_estimate is true.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreviewWithEstimateResult {
    #[serde(flatten)]
    pub(crate) preview: PreviewResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) estimated_size: Option<u64>,
}

/// Unified preview + estimate. Runs both phases with a single progress stream 0-1.
/// Preview uses steps 0..PREVIEW_STEPS, estimate uses steps PREVIEW_STEPS..total.
/// Fetches metadata once to compute accurate total steps (avoids progress bar stuck for short videos).
/// When emit is None, runs silently (e.g. for tests).
pub(crate) async fn run_preview_with_estimate_core(
    input_path: &Path,
    options: &TranscodeOptions,
    preview_start_seconds: Option<f64>,
    emit: PreviewEmit,
) -> Result<PreviewWithEstimateResult, AppError> {
    let meta = get_video_metadata_async(input_path).await?;
    let preview_duration = options.effective_preview_duration() as f64;
    let segment_count = compute_preview_segments(meta.duration, preview_duration).len();
    let total_steps = PREVIEW_STEPS + estimate_step_count(segment_count);
    let emit_ref = emit.as_ref().map(|(a, l)| (a, l.as_str()));

    let (preview_ctx, estimate_ctx) = match emit.as_ref() {
        Some((app, label)) => (
            Some(PreviewProgressCtx::new(
                app.clone(),
                label.clone(),
                0,
                total_steps,
            )),
            Some(PreviewProgressCtx::new(
                app.clone(),
                label.clone(),
                PREVIEW_STEPS,
                total_steps,
            )),
        ),
        None => (None, None),
    };

    let preview_result = run_preview_core(
        input_path,
        options,
        preview_start_seconds,
        emit.clone(),
        preview_ctx,
        Some(meta.duration),
        Some(meta.clone()),
    )
    .await?;

    let input_str = path_to_string(&input_path);
    let preview_duration_u32 = options.effective_preview_duration();
    let file_sig = file_signature(input_path);

    let mut estimated_size = get_cached_estimate(
        &input_str,
        preview_duration_u32,
        options,
        file_sig.as_ref(),
    );
    if estimated_size.is_none() {
        let fresh = compute_estimate_size(
            input_path,
            &input_str,
            preview_duration_u32,
            preview_duration,
            meta.duration,
            options,
            emit_ref,
            estimate_ctx.as_ref(),
        )
        .await?;
        set_cached_estimate(
            &input_str,
            preview_duration_u32,
            options,
            fresh,
            file_sig.as_ref(),
        );
        estimated_size = Some(fresh);
    }

    Ok(PreviewWithEstimateResult {
        preview: preview_result,
        estimated_size: Some(estimated_size.unwrap_or(0)),
    })
}

/// Core preview logic. When emit is Some, emits progress events; when None, runs silently (for tests).
/// When progress_ctx_override is Some, uses it for progress emission (e.g. when part of unified preview+estimate).
/// When video_duration_override is Some, skips ffprobe for input duration.
/// When meta_override is Some, uses it for duration and codec (avoids extra ffprobe when caller already has it).
pub(crate) async fn run_preview_core(
    input_path: &Path,
    options: &TranscodeOptions,
    preview_start_seconds: Option<f64>,
    emit: PreviewEmit,
    progress_ctx_override: Option<PreviewProgressCtx>,
    video_duration_override: Option<f64>,
    meta_override: Option<VideoMetadata>,
) -> Result<PreviewResult, AppError> {
    let input_str = path_to_string(&input_path);
    let preview_duration_u32 = options.effective_preview_duration();
    let preview_duration = preview_duration_u32 as f64;
    let file_sig = file_signature(input_path);
    let emit_ref = emit.as_ref().map(|(a, l)| (a, l.as_str()));
    let progress_ctx = match progress_ctx_override {
        Some(ctx) => Some(ctx),
        None => emit_ref.map(|(app, label)| {
            PreviewProgressCtx::new(app.clone(), label.to_string(), 0, PREVIEW_STEPS)
        }),
    };

    if let Some((app, label)) = emit.as_ref() {
        let _ = app.emit_to(
            label,
            "ffmpeg-progress",
            FfmpegProgressPayload {
                progress: 0.0,
                step: Some("generating_preview".to_string()),
            },
        );
    }

    log::info!(
        target: "tiny_vid::preview",
        "run_preview_core: input={}",
        input_path.display()
    );

    let meta = if let Some(m) = meta_override {
        m
    } else {
        get_video_metadata_async(input_path).await?
    };
    let video_duration = video_duration_override.unwrap_or(meta.duration);
    let codec_playable = meta
        .codec_name
        .as_deref()
        .map(is_browser_playable_codec)
        .unwrap_or(false);
    let preview_start_seconds = clamp_preview_start_seconds(
        preview_start_seconds.unwrap_or(0.0),
        video_duration,
        preview_duration,
    );
    let preview_start_ms = preview_start_ms_from_seconds(preview_start_seconds);

    if let Some((original_path, compressed_path)) = get_cached_preview(
        &input_str,
        preview_duration_u32,
        preview_start_ms,
        options,
        file_sig.as_ref(),
    )
    {
        log::info!(
            target: "tiny_vid::preview",
            "run_preview_core: cache hit, reusing output"
        );
        let start_offset_seconds = get_video_metadata_async(&original_path)
            .await
            .ok()
            .and_then(|m| m.start_time);
        return Ok(PreviewResult {
            original_path: path_to_string(&original_path),
            compressed_path: path_to_string(&compressed_path),
            start_offset_seconds,
        });
    }

    cleanup_previous_preview_paths(&input_str, preview_duration_u32);

    let preview_suffix = "preview-output.mp4";

    let temp = TempFileManager;
    let output_path = temp
        .create(preview_suffix, None)
        .map_err(AppError::from)?;
    let mut cleanup = TempCleanup::new();
    cleanup.add(output_path.clone());

    let preview_segments = vec![(preview_start_seconds, preview_duration)];
    let segment_set = if codec_playable {
        extract_segments_or_use_cache(
            &input_str,
            preview_duration_u32,
            preview_start_ms,
            &preview_segments,
            &temp,
            file_sig.as_ref(),
            emit_ref,
            progress_ctx.as_ref(),
            "preview_extract",
        )
        .await?
    } else {
        // Non-playable codec (ProRes, DNxHD, etc.): transcode segment to H.264/MP4 for display.
        // Segment cache reuses the transcoded original when only options change.
        match get_cached_segments(
            &input_str,
            preview_duration_u32,
            preview_start_ms,
            file_sig.as_ref(),
        ) {
            Some(cached) => {
                log::info!(
                    target: "tiny_vid::preview",
                    "run_preview_core: non-playable segment cache hit, reusing transcoded segment"
                );
                if let Some(ctx) = progress_ctx.as_ref() {
                    let cb = ctx.make_callback("preview_extract");
                    cb(1.0);
                    ctx.advance();
                }
                SegmentSet {
                    paths: cached,
                    created: false,
                }
            }
            None => {
                let orig_path = temp
                    .create("preview-original-transcoded.mp4", None)
                    .map_err(AppError::from)?;
                cleanup.add(orig_path.clone());
                let orig_transcode_opts = TranscodeOptions {
                    codec: Some("libx264".to_string()),
                    quality: Some(90),
                    preset: Some("fast".to_string()),
                    output_format: Some("mp4".to_string()),
                    remove_audio: Some(options.effective_remove_audio()),
                    scale: None, // Preserve original resolution
                    fps: Some(if meta.fps > 0.0 { meta.fps } else { 30.0 }), // Preserve original fps
                    ..TranscodeOptions::default()
                };
                let args = build_ffmpeg_command(
                    &input_str,
                    &path_to_string(&orig_path),
                    &orig_transcode_opts,
                    Some(preview_duration),
                    Some("mp4"),
                    Some(preview_start_seconds),
                )?;
                run_ffmpeg_with_progress(
                    args,
                    Some(preview_duration),
                    emit_ref,
                    progress_ctx.as_ref(),
                    "preview_extract",
                )
                .await?;
                SegmentSet {
                    paths: vec![orig_path],
                    created: true,
                }
            }
        }
    };
    if segment_set.created {
        for path in &segment_set.paths {
            cleanup.add(path.clone());
        }
    }

    transcode_preview_segment(
        &segment_set.paths[0],
        &output_path,
        options,
        Some(preview_duration),
        emit_ref,
        progress_ctx.as_ref(),
    )
    .await?;

    store_preview_paths_for_cleanup(&segment_set.paths, std::slice::from_ref(&output_path));
    set_cached_preview(
        &input_str,
        preview_duration_u32,
        preview_start_ms,
        options,
        segment_set.paths.clone(),
        output_path.clone(),
        file_sig.as_ref(),
    );
    let start_offset_seconds = get_video_metadata_async(&segment_set.paths[0])
        .await
        .ok()
        .and_then(|m| m.start_time);
    log::info!(
        target: "tiny_vid::preview",
        "run_preview_core: complete, start_offset_seconds={:?}",
        start_offset_seconds
    );
    cleanup.keep();
    Ok(PreviewResult {
        original_path: path_to_string(&segment_set.paths[0]),
        compressed_path: path_to_string(&output_path),
        start_offset_seconds,
    })
}

#[cfg(test)]
mod tests {
    use super::{clamp_preview_start_seconds, compute_preview_segments};

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
        assert_eq!(segs[0], (0.0, 3.0), "first segment samples start");
        assert_eq!(segs[1], (29.5, 1.0));
        assert_eq!(segs[2], (59.0, 1.0));
    }

    #[test]
    fn estimate_segments_sample_begin_mid_end() {
        let segs = compute_preview_segments(10.0, 3.0);
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], (0.0, 3.0));
        assert_eq!(segs[1], (4.5, 1.0));
        assert_eq!(segs[2], (9.0, 1.0));
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

    #[test]
    fn clamp_preview_start_when_past_end() {
        let clamped = clamp_preview_start_seconds(8.0, 10.0, 3.0);
        assert_eq!(clamped, 7.0);
    }

    #[test]
    fn clamp_preview_start_when_preview_longer_than_video() {
        let clamped = clamp_preview_start_seconds(2.0, 1.5, 3.0);
        assert_eq!(clamped, 0.0);
    }

    #[test]
    fn clamp_preview_start_returns_zero_when_duration_invalid() {
        let clamped = clamp_preview_start_seconds(5.0, 0.0, 3.0);
        assert_eq!(clamped, 0.0, "video_duration <= 0 should return 0");
        let clamped2 = clamp_preview_start_seconds(5.0, 10.0, 0.0);
        assert_eq!(clamped2, 0.0, "preview_duration <= 0 should return 0");
    }
}
