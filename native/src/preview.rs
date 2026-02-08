//! Preview generation for video compression.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::error::AppError;
use crate::ffmpeg::ffprobe::{VideoMetadata, get_video_metadata_impl};
use crate::ffmpeg::{
    EstimateConfidence, FfmpegProgressPayload, FileSignature, SizeEstimate, TempFileManager,
    TranscodeOptions, build_extract_args, build_ffmpeg_command, cleanup_previous_preview_paths,
    file_signature, get_cached_estimate, get_cached_preview, get_cached_segments,
    is_preview_stream_copy_safe_codec, path_to_string, run_ffmpeg_blocking, set_cached_estimate,
    set_cached_preview, store_preview_paths_for_cleanup,
};

pub(crate) type PreviewProgressEmit = Arc<dyn Fn(FfmpegProgressPayload) + Send + Sync>;

/// Step counts for progress emission. Preview: extract + transcode. Estimate: up to 5 sample encodes.
const PREVIEW_STEPS: usize = 2;
const ESTIMATE_SHORT_VIDEO_THRESHOLD_SECS: f64 = 12.0;
const ESTIMATE_ADAPTIVE_MIN_DURATION_SECS: f64 = 30.0;
const ESTIMATE_BASE_SAMPLE_DURATION_SECS: f64 = 1.5;
const ESTIMATE_MAX_SAMPLED_SECONDS: f64 = 7.5;
const ESTIMATE_EXTRA_SAMPLE_CV_THRESHOLD: f64 = 0.35;
const ESTIMATE_HIGH_CONFIDENCE_MAX_CV: f64 = 0.15;
const ESTIMATE_MEDIUM_CONFIDENCE_MAX_CV: f64 = 0.35;
const ESTIMATE_METHOD: &str = "sampled_bitrate";

fn estimate_step_count(video_duration: f64) -> usize {
    if video_duration > ESTIMATE_SHORT_VIDEO_THRESHOLD_SECS {
        5
    } else {
        1
    }
}

/// Progress context for multi-step preview (extract + transcode + estimate).
pub(crate) struct PreviewProgressCtx {
    emit_progress: PreviewProgressEmit,
    step_index: AtomicUsize,
    base_step: usize,
    total_steps: usize,
}

impl PreviewProgressCtx {
    pub(crate) fn new(
        emit_progress: PreviewProgressEmit,
        base_step: usize,
        total_steps: usize,
    ) -> Self {
        Self {
            emit_progress,
            step_index: AtomicUsize::new(0),
            base_step,
            total_steps,
        }
    }

    fn make_callback(&self, step: &'static str) -> Arc<dyn Fn(f64) + Send + Sync> {
        let idx = self.step_index.load(Ordering::Relaxed);
        let emit_progress = Arc::clone(&self.emit_progress);
        let base = self.base_step as f64;
        let total = self.total_steps as f64;
        let step_owned = step.to_string();
        Arc::new(move |p: f64| {
            let overall = (base + idx as f64 + p) / total;
            let payload = FfmpegProgressPayload {
                progress: overall,
                step: Some(step_owned.clone()),
            };
            emit_progress(payload);
        })
    }

    fn advance(&self) {
        self.step_index.fetch_add(1, Ordering::Relaxed);
    }
}

pub(crate) fn make_preview_progress_ctx(emit_progress: PreviewProgressEmit) -> PreviewProgressCtx {
    PreviewProgressCtx::new(emit_progress, 0, PREVIEW_STEPS)
}

/// Runs FFmpeg with optional progress callback.
pub(crate) async fn run_ffmpeg_step(
    args: Vec<String>,
    duration_secs: Option<f64>,
    progress_callback: Option<std::sync::Arc<dyn Fn(f64) + Send + Sync>>,
) -> Result<(), AppError> {
    let result = tokio::task::spawn_blocking(move || {
        run_ffmpeg_blocking(args, duration_secs, progress_callback, None)
    })
    .await;

    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(join_err) => Err(AppError::from(join_err.to_string())),
    }
}

async fn run_ffmpeg_with_progress(
    args: Vec<String>,
    duration_secs: Option<f64>,
    progress_ctx: Option<&PreviewProgressCtx>,
    step_label: &'static str,
) -> Result<(), AppError> {
    let progress_cb = progress_ctx.map(|ctx| ctx.make_callback(step_label));
    run_ffmpeg_step(args, duration_secs, progress_cb).await?;
    if let Some(ctx) = progress_ctx {
        ctx.advance();
    }
    Ok(())
}

fn complete_progress_steps(
    progress_ctx: Option<&PreviewProgressCtx>,
    count: usize,
    step_label: &'static str,
) {
    let Some(ctx) = progress_ctx else {
        return;
    };
    for _ in 0..count {
        let cb = ctx.make_callback(step_label);
        cb(1.0);
        ctx.advance();
    }
}

fn clamp_preview_start_seconds(requested: f64, video_duration: f64, preview_duration: f64) -> f64 {
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

fn preview_original_transcode_codec() -> &'static str {
    #[cfg(feature = "lgpl")]
    {
        "h264_videotoolbox"
    }
    #[cfg(not(feature = "lgpl"))]
    {
        "libx264"
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct EstimateSampleWindow {
    start_seconds: f64,
    duration_seconds: f64,
}

struct OriginalPreviewTranscodeCtx<'a> {
    input_str: &'a str,
    preview_duration_u32: u32,
    preview_start_ms: u64,
    preview_start_seconds: f64,
    preview_duration: f64,
    source_fps: f64,
    remove_audio: bool,
    temp: &'a TempFileManager,
    file_signature: Option<&'a FileSignature>,
    progress_ctx: Option<&'a PreviewProgressCtx>,
}

async fn get_video_metadata_async(path: &Path) -> Result<VideoMetadata, AppError> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || get_video_metadata_impl(&path))
        .await
        .map_err(|e| AppError::from(e.to_string()))?
}

/// Extracts preview segments from input, or returns cached segment paths if available.
/// strip_audio: when true, copy only video (-map 0:v -c:v copy -an).
/// step_label: when progress_ctx is Some, label for progress ("extract" or "estimate").
async fn extract_segments_or_use_cache(
    input_str: &str,
    preview_duration_u32: u32,
    preview_start_ms: u64,
    segments: &[(f64, f64)],
    temp: &TempFileManager,
    file_signature: Option<&FileSignature>,
    progress_ctx: Option<&PreviewProgressCtx>,
    step_label: &'static str,
    strip_audio: bool,
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
                let args = build_extract_args(
                    input_str,
                    *start,
                    *dur,
                    &path_to_string(path),
                    strip_audio,
                );
                if let Err(err) =
                    run_ffmpeg_with_progress(args, Some(*dur), progress_ctx, step_label).await
                {
                    for created in &paths {
                        let _ = fs::remove_file(created);
                    }
                    return Err(err);
                }
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
    progress_ctx: Option<&PreviewProgressCtx>,
) -> Result<(), AppError> {
    let preview_opts = preview_transcode_options(options);
    let args = build_ffmpeg_command(
        &path_to_string(segment_path),
        &path_to_string(output_path),
        &preview_opts,
        output_duration,
        Some("mp4"),
        None,
    )?;

    run_ffmpeg_with_progress(args, output_duration, progress_ctx, "preview_transcode").await?;
    Ok(())
}

fn preview_transcode_options(options: &TranscodeOptions) -> TranscodeOptions {
    let mut preview_opts = options.clone();
    preview_opts.remove_audio = Some(true);
    preview_opts
}

async fn transcode_original_preview_segment_or_use_cache(
    ctx: OriginalPreviewTranscodeCtx<'_>,
) -> Result<SegmentSet, AppError> {
    match get_cached_segments(
        ctx.input_str,
        ctx.preview_duration_u32,
        ctx.preview_start_ms,
        ctx.file_signature,
    ) {
        Some(cached) => {
            log::info!(
                target: "tiny_vid::preview",
                "transcode_original_preview_segment_or_use_cache: cache hit, reusing transcoded segment"
            );
            if let Some(progress_ctx) = ctx.progress_ctx {
                let cb = progress_ctx.make_callback("preview_extract");
                cb(1.0);
                progress_ctx.advance();
            }
            Ok(SegmentSet {
                paths: cached,
                created: false,
            })
        }
        None => {
            let orig_path = ctx
                .temp
                .create("preview-original-transcoded.mp4", None)
                .map_err(AppError::from)?;
            let orig_transcode_opts = preview_transcode_options(&TranscodeOptions {
                codec: Some(preview_original_transcode_codec().to_string()),
                quality: Some(90),
                preset: Some("fast".to_string()),
                output_format: Some("mp4".to_string()),
                remove_audio: Some(ctx.remove_audio),
                scale: None,
                fps: Some(if ctx.source_fps > 0.0 {
                    ctx.source_fps
                } else {
                    30.0
                }),
                ..TranscodeOptions::default()
            });
            let args = build_ffmpeg_command(
                ctx.input_str,
                &path_to_string(&orig_path),
                &orig_transcode_opts,
                Some(ctx.preview_duration),
                Some("mp4"),
                Some(ctx.preview_start_seconds),
            )?;
            if let Err(err) = run_ffmpeg_with_progress(
                args,
                Some(ctx.preview_duration),
                ctx.progress_ctx,
                "preview_extract",
            )
            .await
            {
                let _ = fs::remove_file(&orig_path);
                return Err(err);
            }
            Ok(SegmentSet {
                paths: vec![orig_path],
                created: true,
            })
        }
    }
}

fn clamp_sample_start(center: f64, sample_duration: f64, video_duration: f64) -> f64 {
    let max_start = (video_duration - sample_duration).max(0.0);
    (center - (sample_duration / 2.0)).clamp(0.0, max_start)
}

fn sample_duration_for_video(video_duration: f64) -> f64 {
    ESTIMATE_BASE_SAMPLE_DURATION_SECS.min(video_duration.max(0.1))
}

fn sample_at_percent(
    video_duration: f64,
    sample_duration: f64,
    percent: f64,
) -> EstimateSampleWindow {
    EstimateSampleWindow {
        start_seconds: clamp_sample_start(
            video_duration * percent,
            sample_duration,
            video_duration,
        ),
        duration_seconds: sample_duration,
    }
}

fn base_estimate_samples(video_duration: f64) -> Vec<EstimateSampleWindow> {
    if video_duration <= 0.0 {
        return vec![];
    }
    if video_duration <= ESTIMATE_SHORT_VIDEO_THRESHOLD_SECS {
        return vec![EstimateSampleWindow {
            start_seconds: 0.0,
            duration_seconds: video_duration,
        }];
    }
    let sample_duration = sample_duration_for_video(video_duration);
    vec![
        sample_at_percent(video_duration, sample_duration, 0.05),
        sample_at_percent(video_duration, sample_duration, 0.50),
        sample_at_percent(video_duration, sample_duration, 0.95),
    ]
}

fn extra_estimate_samples(video_duration: f64) -> Vec<EstimateSampleWindow> {
    let sample_duration = sample_duration_for_video(video_duration);
    vec![
        sample_at_percent(video_duration, sample_duration, 0.25),
        sample_at_percent(video_duration, sample_duration, 0.75),
    ]
}

fn coefficient_of_variation(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    if mean <= 0.0 {
        return 0.0;
    }
    let variance = values
        .iter()
        .map(|v| {
            let diff = *v - mean;
            diff * diff
        })
        .sum::<f64>()
        / values.len() as f64;
    variance.sqrt() / mean
}

fn aggregate_bytes_per_sec(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    if values.len() < 5 {
        return Some(values.iter().sum::<f64>() / values.len() as f64);
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let trimmed = &sorted[1..sorted.len() - 1];
    Some(trimmed.iter().sum::<f64>() / trimmed.len() as f64)
}

fn confidence_band_for_cv(cv: f64) -> (EstimateConfidence, f64) {
    if cv <= ESTIMATE_HIGH_CONFIDENCE_MAX_CV {
        (EstimateConfidence::High, 0.08)
    } else if cv <= ESTIMATE_MEDIUM_CONFIDENCE_MAX_CV {
        (EstimateConfidence::Medium, 0.15)
    } else {
        (EstimateConfidence::Low, 0.30)
    }
}

async fn encode_estimate_sample(
    input_path: &Path,
    options: &TranscodeOptions,
    sample: EstimateSampleWindow,
    sample_index: usize,
    cleanup: &mut TempCleanup,
    progress_ctx: Option<&PreviewProgressCtx>,
) -> Result<f64, AppError> {
    if sample.duration_seconds <= 0.0 {
        return Err(AppError::from(
            "Estimate sample duration must be greater than zero",
        ));
    }
    let output_format = options.effective_output_format();
    let output_path = TempFileManager
        .create(
            &format!("preview-estimate-{}.{}", sample_index, output_format),
            None,
        )
        .map_err(AppError::from)?;
    cleanup.add(output_path.clone());

    let args = build_ffmpeg_command(
        &path_to_string(input_path),
        &path_to_string(&output_path),
        options,
        Some(sample.duration_seconds),
        None,
        Some(sample.start_seconds),
    )?;
    run_ffmpeg_with_progress(
        args,
        Some(sample.duration_seconds),
        progress_ctx,
        "preview_estimate",
    )
    .await?;

    let output_size = fs::metadata(&output_path)?.len() as f64;
    Ok(output_size / sample.duration_seconds.max(0.001))
}

async fn compute_estimate_size(
    input_path: &Path,
    video_duration: f64,
    options: &TranscodeOptions,
    progress_ctx: Option<&PreviewProgressCtx>,
) -> Result<SizeEstimate, AppError> {
    if !video_duration.is_finite() || video_duration <= 0.0 {
        return Err(AppError::from("Invalid video duration for size estimation"));
    }

    let input_size = fs::metadata(input_path)?.len();
    let max_reasonable = input_size.saturating_mul(2);

    let base_samples = base_estimate_samples(video_duration);
    if base_samples.is_empty() {
        return Err(AppError::from("No estimate samples were planned"));
    }
    let mut remaining_extra_steps = if base_samples.len() == 3 {
        estimate_step_count(video_duration).saturating_sub(base_samples.len())
    } else {
        0
    };

    let mut cleanup = TempCleanup::new();
    let mut sample_rates = Vec::new();
    let mut sample_seconds_total = 0.0;
    let mut sample_index = 0usize;

    for sample in &base_samples {
        let bytes_per_sec = encode_estimate_sample(
            input_path,
            options,
            *sample,
            sample_index,
            &mut cleanup,
            progress_ctx,
        )
        .await?;
        sample_rates.push(bytes_per_sec);
        sample_seconds_total += sample.duration_seconds;
        sample_index += 1;
    }

    let base_cv = coefficient_of_variation(&sample_rates);
    let should_add_extra_samples = video_duration >= ESTIMATE_ADAPTIVE_MIN_DURATION_SECS
        && base_cv > ESTIMATE_EXTRA_SAMPLE_CV_THRESHOLD
        && sample_seconds_total < ESTIMATE_MAX_SAMPLED_SECONDS;
    if should_add_extra_samples {
        for sample in extra_estimate_samples(video_duration) {
            if sample_seconds_total + sample.duration_seconds > ESTIMATE_MAX_SAMPLED_SECONDS {
                break;
            }
            let bytes_per_sec = encode_estimate_sample(
                input_path,
                options,
                sample,
                sample_index,
                &mut cleanup,
                progress_ctx,
            )
            .await?;
            sample_rates.push(bytes_per_sec);
            sample_seconds_total += sample.duration_seconds;
            sample_index += 1;
            remaining_extra_steps = remaining_extra_steps.saturating_sub(1);
        }
    }
    complete_progress_steps(progress_ctx, remaining_extra_steps, "preview_estimate");

    let aggregate_bps = aggregate_bytes_per_sec(&sample_rates)
        .ok_or_else(|| AppError::from("Unable to aggregate estimate sample bitrates"))?;
    let best_size = ((aggregate_bps * video_duration).max(0.0) as u64).min(max_reasonable);
    let cv = coefficient_of_variation(&sample_rates);
    let (confidence, band) = confidence_band_for_cv(cv);
    let low_size = ((best_size as f64 * (1.0 - band)).max(0.0) as u64).min(best_size);
    let high_size = ((best_size as f64 * (1.0 + band)) as u64)
        .max(best_size)
        .min(max_reasonable);

    Ok(SizeEstimate {
        best_size,
        low_size,
        high_size,
        confidence,
        method: ESTIMATE_METHOD.to_string(),
        sample_count: sample_rates.len() as u32,
        sample_seconds_total,
    })
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
    pub(crate) estimate: Option<SizeEstimate>,
}

/// Unified preview + estimate. Runs both phases with a single progress stream 0-1.
/// Preview uses steps 0..PREVIEW_STEPS, estimate uses steps PREVIEW_STEPS..total.
/// Fetches metadata once to compute accurate total steps (avoids progress bar stuck for short videos).
#[cfg_attr(not(feature = "integration-test-api"), allow(dead_code))]
pub(crate) async fn run_preview_with_estimate_core(
    input_path: &Path,
    options: &TranscodeOptions,
    preview_start_seconds: Option<f64>,
) -> Result<PreviewWithEstimateResult, AppError> {
    run_preview_with_estimate_core_with_progress(input_path, options, preview_start_seconds, None)
        .await
}

pub(crate) async fn run_preview_with_estimate_core_with_progress(
    input_path: &Path,
    options: &TranscodeOptions,
    preview_start_seconds: Option<f64>,
    progress_emit: Option<PreviewProgressEmit>,
) -> Result<PreviewWithEstimateResult, AppError> {
    let meta = get_video_metadata_async(input_path).await?;
    let estimate_steps = estimate_step_count(meta.duration);
    let total_steps = PREVIEW_STEPS + estimate_steps;

    let (preview_ctx, estimate_ctx) = match progress_emit {
        Some(emit_progress) => (
            Some(PreviewProgressCtx::new(
                Arc::clone(&emit_progress),
                0,
                total_steps,
            )),
            Some(PreviewProgressCtx::new(
                emit_progress,
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
        preview_ctx,
        Some(meta.duration),
        Some(meta.clone()),
    )
    .await?;

    let input_str = path_to_string(&input_path);
    let preview_duration_u32 = options.effective_preview_duration();
    let file_sig = file_signature(input_path);

    let mut estimate =
        get_cached_estimate(&input_str, preview_duration_u32, options, file_sig.as_ref());
    if estimate.is_some() {
        complete_progress_steps(estimate_ctx.as_ref(), estimate_steps, "preview_estimate");
    } else {
        match compute_estimate_size(input_path, meta.duration, options, estimate_ctx.as_ref()).await
        {
            Ok(fresh) => {
                set_cached_estimate(
                    &input_str,
                    preview_duration_u32,
                    options,
                    fresh.clone(),
                    file_sig.as_ref(),
                );
                estimate = Some(fresh);
            }
            Err(err) => {
                log::warn!(
                    target: "tiny_vid::preview",
                    "run_preview_with_estimate_core: failed to compute estimate: {}",
                    err
                );
                estimate = None;
            }
        }
    }

    Ok(PreviewWithEstimateResult {
        preview: preview_result,
        estimate,
    })
}

/// `progress_ctx_override`: when Some, uses it for progress (e.g. unified preview+estimate).
/// `video_duration_override` / `meta_override`: when Some, skip ffprobe when caller already has it.
pub(crate) async fn run_preview_core(
    input_path: &Path,
    options: &TranscodeOptions,
    preview_start_seconds: Option<f64>,
    progress_ctx_override: Option<PreviewProgressCtx>,
    video_duration_override: Option<f64>,
    meta_override: Option<VideoMetadata>,
) -> Result<PreviewResult, AppError> {
    let input_str = path_to_string(&input_path);
    let preview_duration_u32 = options.effective_preview_duration();
    let preview_duration = preview_duration_u32 as f64;
    let file_sig = file_signature(input_path);
    let progress_ctx = progress_ctx_override;

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
    let preview_opts = preview_transcode_options(options);
    let video_duration = video_duration_override.unwrap_or(meta.duration);
    let source_codec = meta.codec_name.as_deref().unwrap_or("unknown");
    let can_stream_copy_video = is_preview_stream_copy_safe_codec(source_codec);
    let can_stream_copy_original_preview = can_stream_copy_video;
    log::info!(
        target: "tiny_vid::preview",
        "run_preview_core: stream-copy policy (video_codec={}, video_safe={}, can_stream_copy={})",
        source_codec,
        can_stream_copy_video,
        can_stream_copy_original_preview
    );
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
        &preview_opts,
        file_sig.as_ref(),
    ) {
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
    let output_path = temp.create(preview_suffix, None).map_err(AppError::from)?;
    let mut cleanup = TempCleanup::new();
    cleanup.add(output_path.clone());

    let preview_segments = vec![(preview_start_seconds, preview_duration)];
    let segment_set = if can_stream_copy_original_preview {
        match extract_segments_or_use_cache(
            &input_str,
            preview_duration_u32,
            preview_start_ms,
            &preview_segments,
            &temp,
            file_sig.as_ref(),
            progress_ctx.as_ref(),
            "preview_extract",
            true,
        )
        .await
        {
            Ok(segment_set) => segment_set,
            Err(err) => {
                log::warn!(
                    target: "tiny_vid::preview",
                    "run_preview_core: stream-copy preview extract failed, retrying with H.264 transcode: {}",
                    err
                );
                log::info!(
                    target: "tiny_vid::preview",
                    "run_preview_core: stream-copy unavailable, using transcode fallback (video_codec={}, reason=extract_failed)",
                    source_codec
                );
                transcode_original_preview_segment_or_use_cache(OriginalPreviewTranscodeCtx {
                    input_str: &input_str,
                    preview_duration_u32,
                    preview_start_ms,
                    preview_start_seconds,
                    preview_duration,
                    source_fps: meta.fps,
                    remove_audio: true,
                    temp: &temp,
                    file_signature: file_sig.as_ref(),
                    progress_ctx: progress_ctx.as_ref(),
                })
                .await?
            }
        }
    } else {
        log::info!(
            target: "tiny_vid::preview",
            "run_preview_core: stream-copy unavailable, using transcode fallback (video_codec={}, reason=unsupported_stream_policy)",
            source_codec
        );
        transcode_original_preview_segment_or_use_cache(OriginalPreviewTranscodeCtx {
            input_str: &input_str,
            preview_duration_u32,
            preview_start_ms,
            preview_start_seconds,
            preview_duration,
            source_fps: meta.fps,
            remove_audio: true,
            temp: &temp,
            file_signature: file_sig.as_ref(),
            progress_ctx: progress_ctx.as_ref(),
        })
        .await?
    };
    if segment_set.created {
        for path in &segment_set.paths {
            cleanup.add(path.clone());
        }
    }

    transcode_preview_segment(
        &segment_set.paths[0],
        &output_path,
        &preview_opts,
        None,
        progress_ctx.as_ref(),
    )
    .await?;

    store_preview_paths_for_cleanup(&segment_set.paths, std::slice::from_ref(&output_path));
    set_cached_preview(
        &input_str,
        preview_duration_u32,
        preview_start_ms,
        &preview_opts,
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
    use super::{
        ESTIMATE_BASE_SAMPLE_DURATION_SECS, EstimateConfidence, EstimateSampleWindow,
        base_estimate_samples, clamp_preview_start_seconds, coefficient_of_variation,
        confidence_band_for_cv,
    };

    #[test]
    fn base_estimate_samples_short_video_uses_single_full_sample() {
        let segs = base_estimate_samples(10.0);
        assert_eq!(
            segs,
            vec![EstimateSampleWindow {
                start_seconds: 0.0,
                duration_seconds: 10.0
            }]
        );
    }

    #[test]
    fn base_estimate_samples_long_video_uses_three_positions() {
        let segs = base_estimate_samples(60.0);
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0].duration_seconds, ESTIMATE_BASE_SAMPLE_DURATION_SECS);
        assert!(segs[0].start_seconds >= 0.0);
        assert!(segs[2].start_seconds >= segs[1].start_seconds);
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

    #[test]
    fn coefficient_of_variation_is_zero_for_uniform_values() {
        let cv = coefficient_of_variation(&[10.0, 10.0, 10.0]);
        assert_eq!(cv, 0.0);
    }

    #[test]
    fn coefficient_of_variation_is_positive_for_varied_values() {
        let cv = coefficient_of_variation(&[10.0, 20.0, 30.0]);
        assert!(cv > 0.0);
    }

    #[test]
    fn confidence_mapping_uses_cv_buckets() {
        let (high, _) = confidence_band_for_cv(0.10);
        let (medium, _) = confidence_band_for_cv(0.20);
        let (low, _) = confidence_band_for_cv(0.40);
        assert_eq!(high, EstimateConfidence::High);
        assert_eq!(medium, EstimateConfidence::Medium);
        assert_eq!(low, EstimateConfidence::Low);
    }
}
