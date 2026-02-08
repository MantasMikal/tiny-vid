use std::collections::HashMap;
use std::fs;
use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use parking_lot::Mutex;

use crate::codec::BuildVariantResult;
use crate::error::AppError;
use crate::ffmpeg::ffprobe::{VideoMetadata as FfprobeVideoMetadata, get_video_metadata_impl};
use crate::ffmpeg::{
    FfmpegProgressPayload, TempFileManager, TranscodeOptions, build_ffmpeg_command,
    cleanup_transcode_temp, format_args_for_display_multiline, path_to_string, run_ffmpeg_blocking,
    set_transcode_temp, terminate_all_ffmpeg,
};
use crate::preview::{
    PreviewProgressEmit, make_preview_progress_ctx, run_preview_core,
    run_preview_with_estimate_core_with_progress,
};

pub type SidecarProgressEmitter = Arc<dyn Fn(FfmpegProgressPayload) + Send + Sync>;

const COMMIT_TOKEN_MAX_AGE: Duration = Duration::from_secs(24 * 3600);
const PROTOCOL_VERSION: u8 = 2;

#[derive(Debug, Clone)]
struct PendingTranscode {
    path: PathBuf,
    created_at: SystemTime,
}

static NEXT_COMMIT_TOKEN_ID: AtomicU64 = AtomicU64::new(1);
static PENDING_TRANSCODES: std::sync::LazyLock<Mutex<HashMap<String, PendingTranscode>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppCapabilitiesResult {
    pub protocol_version: u8,
    pub variant: &'static str,
    pub codecs: Vec<crate::CodecInfo>,
}

fn block_on_async<T>(future: impl Future<Output = Result<T, AppError>>) -> Result<T, AppError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| AppError::from(format!("Failed to initialize async runtime: {}", e)))?;
    runtime.block_on(future)
}

fn is_cross_device_rename_error(e: &io::Error) -> bool {
    #[cfg(unix)]
    {
        e.raw_os_error() == Some(18) // EXDEV
    }
    #[cfg(windows)]
    {
        e.raw_os_error() == Some(17) // ERROR_NOT_SAME_DEVICE
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = e;
        false
    }
}

fn make_commit_token() -> String {
    let id = NEXT_COMMIT_TOKEN_ID.fetch_add(1, Ordering::Relaxed);
    format!(
        "commit-{}-{}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        id
    )
}

fn remove_expired_pending_transcodes(
    map: &mut HashMap<String, PendingTranscode>,
    max_age: Duration,
) {
    let now = SystemTime::now();
    let expired_tokens: Vec<String> = map
        .iter()
        .filter_map(|(token, entry)| {
            let age = now.duration_since(entry.created_at).unwrap_or_default();
            if age > max_age {
                Some(token.clone())
            } else {
                None
            }
        })
        .collect();

    for token in expired_tokens {
        if let Some(entry) = map.remove(&token) {
            let _ = fs::remove_file(entry.path);
        }
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoMetadataResult {
    duration: f64,
    width: u32,
    height: u32,
    size: u64,
    size_mb: f64,
    fps: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    codec_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    codec_long_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    video_bit_rate: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    format_bit_rate: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    format_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    format_long_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nb_streams: Option<u32>,
    audio_stream_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    subtitle_stream_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audio_codec_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audio_channels: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    encoder: Option<String>,
}

impl From<FfprobeVideoMetadata> for VideoMetadataResult {
    fn from(meta: FfprobeVideoMetadata) -> Self {
        let fps = (meta.fps * 100.0).round() / 100.0;
        Self {
            duration: meta.duration,
            width: meta.width,
            height: meta.height,
            size: meta.size,
            size_mb: meta.size as f64 / 1024.0 / 1024.0,
            fps,
            codec_name: meta.codec_name,
            codec_long_name: meta.codec_long_name,
            video_bit_rate: meta.video_bit_rate,
            format_bit_rate: meta.format_bit_rate,
            format_name: meta.format_name,
            format_long_name: meta.format_long_name,
            nb_streams: meta.nb_streams,
            audio_stream_count: meta.audio_stream_count,
            subtitle_stream_count: Some(meta.subtitle_stream_count),
            audio_codec_name: meta.audio_codec_name,
            encoder: meta.encoder,
            audio_channels: meta.audio_channels,
        }
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewWithEstimateResult {
    pub original_path: String,
    pub compressed_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_offset_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimate: Option<crate::ffmpeg::SizeEstimate>,
}

pub fn ffmpeg_transcode_to_temp_with_events(
    input_path: PathBuf,
    options: TranscodeOptions,
    event_emitter: Option<SidecarProgressEmitter>,
) -> Result<String, AppError> {
    cleanup_transcode_temp();

    let ext = options.effective_output_format();
    let suffix = format!("transcode-output.{}", ext);

    let temp = TempFileManager;
    let output_path = temp.create(&suffix, None).map_err(AppError::from)?;
    let output_str = path_to_string(&output_path);

    set_transcode_temp(Some(output_path.clone()));

    let args = build_ffmpeg_command(
        &path_to_string(&input_path),
        &output_str,
        &options,
        None,
        None,
        None,
    )?;
    let duration_secs = options.duration_secs;
    let progress_callback = event_emitter.as_ref().map(|emit| {
        let emit = Arc::clone(emit);
        Arc::new(move |progress: f64| {
            emit(FfmpegProgressPayload {
                progress,
                step: Some("transcode".to_string()),
            });
        }) as Arc<dyn Fn(f64) + Send + Sync>
    });

    match run_ffmpeg_blocking(args, duration_secs, progress_callback, None) {
        Ok(()) => Ok(output_str),
        Err(e) => {
            cleanup_transcode_temp();
            Err(e)
        }
    }
}

pub fn ffmpeg_preview_with_events(
    input_path: PathBuf,
    options: TranscodeOptions,
    preview_start_seconds: Option<f64>,
    include_estimate: bool,
    event_emitter: Option<SidecarProgressEmitter>,
) -> Result<PreviewWithEstimateResult, AppError> {
    if let Some(emit) = event_emitter.as_ref() {
        emit(FfmpegProgressPayload {
            progress: 0.0,
            step: Some("generating_preview".to_string()),
        });
    }

    let preview_progress_emit: Option<PreviewProgressEmit> = event_emitter.as_ref().map(|emit| {
        let emit = Arc::clone(emit);
        Arc::new(move |payload: FfmpegProgressPayload| {
            emit(payload);
        }) as PreviewProgressEmit
    });

    let result = block_on_async(async {
        if include_estimate {
            let result = run_preview_with_estimate_core_with_progress(
                &input_path,
                &options,
                preview_start_seconds,
                preview_progress_emit.clone(),
            )
            .await?;
            Ok(PreviewWithEstimateResult {
                original_path: result.preview.original_path,
                compressed_path: result.preview.compressed_path,
                start_offset_seconds: result.preview.start_offset_seconds,
                estimate: result.estimate,
            })
        } else {
            let progress_ctx = preview_progress_emit.clone().map(make_preview_progress_ctx);
            let result = run_preview_core(
                &input_path,
                &options,
                preview_start_seconds,
                progress_ctx,
                None,
                None,
            )
            .await?;
            Ok(PreviewWithEstimateResult {
                original_path: result.original_path,
                compressed_path: result.compressed_path,
                start_offset_seconds: result.start_offset_seconds,
                estimate: None,
            })
        }
    });

    if result.is_ok()
        && let Some(emit) = event_emitter.as_ref()
    {
        emit(FfmpegProgressPayload {
            progress: 1.0,
            step: Some("preview_complete".to_string()),
        });
    }

    result
}

pub fn get_video_metadata(path: PathBuf) -> Result<VideoMetadataResult, AppError> {
    let meta = get_video_metadata_impl(&path)?;
    Ok(meta.into())
}

pub fn preview_ffmpeg_command(options: TranscodeOptions, input_path: Option<String>) -> String {
    let input_str = input_path.as_deref().unwrap_or("<input>");
    let output_str = "<output>";
    let args = build_ffmpeg_command(input_str, output_str, &options, None, None, None)
        .unwrap_or_else(|e| vec!["# error".into(), e.to_string()]);
    format!("ffmpeg\n{}", format_args_for_display_multiline(&args))
}

pub fn ffmpeg_terminate() {
    terminate_all_ffmpeg();
}

fn move_compressed_file(source: PathBuf, dest: PathBuf) -> Result<(), AppError> {
    match fs::rename(&source, &dest) {
        Ok(()) => Ok(()),
        Err(e) => {
            if is_cross_device_rename_error(&e) {
                fs::copy(&source, &dest)?;
                fs::remove_file(&source)?;
                return Ok(());
            }
            Err(e.into())
        }
    }
}

fn detect_build_variant() -> Result<BuildVariantResult, AppError> {
    let available = crate::ffmpeg::discovery::get_available_codecs()?;
    crate::codec::get_build_variant(available)
}

pub fn app_capabilities() -> Result<AppCapabilitiesResult, AppError> {
    let variant = detect_build_variant()?;
    Ok(AppCapabilitiesResult {
        protocol_version: PROTOCOL_VERSION,
        variant: variant.variant,
        codecs: variant.codecs,
    })
}

pub fn register_transcode_commit(output_path: PathBuf) -> Result<String, AppError> {
    if !output_path.exists() {
        return Err(AppError::from(format!(
            "Transcode output does not exist: {}",
            output_path.display()
        )));
    }

    let token = make_commit_token();
    let mut guard = PENDING_TRANSCODES.lock();
    remove_expired_pending_transcodes(&mut guard, COMMIT_TOKEN_MAX_AGE);
    guard.insert(
        token.clone(),
        PendingTranscode {
            path: output_path,
            created_at: SystemTime::now(),
        },
    );
    // The output is now tracked by commit-token lifecycle, not active transcode slot.
    set_transcode_temp(None);
    Ok(token)
}

pub fn commit_transcode_output(commit_token: String, dest: PathBuf) -> Result<String, AppError> {
    let mut guard = PENDING_TRANSCODES.lock();
    remove_expired_pending_transcodes(&mut guard, COMMIT_TOKEN_MAX_AGE);

    let Some(entry) = guard.remove(&commit_token) else {
        return Err(AppError::from(format!(
            "Unknown commitToken: {}",
            commit_token
        )));
    };
    drop(guard);

    match move_compressed_file(entry.path.clone(), dest.clone()) {
        Ok(()) => Ok(path_to_string(&dest)),
        Err(error) => {
            // Preserve retry semantics if move fails.
            let mut guard = PENDING_TRANSCODES.lock();
            guard.insert(commit_token, entry);
            Err(error)
        }
    }
}

pub fn discard_transcode_output(commit_token: String) -> Result<(), AppError> {
    let mut guard = PENDING_TRANSCODES.lock();
    remove_expired_pending_transcodes(&mut guard, COMMIT_TOKEN_MAX_AGE);

    let Some(entry) = guard.remove(&commit_token) else {
        return Err(AppError::from(format!(
            "Unknown commitToken: {}",
            commit_token
        )));
    };
    drop(guard);

    let _ = fs::remove_file(entry.path);
    Ok(())
}

pub fn cleanup_pending_transcodes(max_age: Duration) {
    let mut guard = PENDING_TRANSCODES.lock();
    remove_expired_pending_transcodes(&mut guard, max_age);
}

pub fn cleanup_all_pending_transcodes() {
    let mut guard = PENDING_TRANSCODES.lock();
    for (_, entry) in guard.drain() {
        let _ = fs::remove_file(entry.path);
    }
}

pub fn cleanup_on_exit() {
    crate::ffmpeg::cleanup_transcode_temp();
    cleanup_all_pending_transcodes();
    crate::ffmpeg::cleanup_preview_transcode_cache();
}

pub fn cleanup_startup_temp(max_age: std::time::Duration) {
    crate::ffmpeg::cleanup_old_temp_files(max_age);
    cleanup_pending_transcodes(max_age);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn clear_pending_for_test() {
        let mut guard = PENDING_TRANSCODES.lock();
        for (_, entry) in guard.drain() {
            let _ = fs::remove_file(entry.path);
        }
    }

    #[test]
    #[serial]
    fn commit_token_commit_moves_file() {
        clear_pending_for_test();
        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("source.mp4");
        fs::write(&source, b"video").expect("write source");

        let token = register_transcode_commit(source.clone()).expect("register token");
        let dest = dir.path().join("dest.mp4");
        let saved_path = commit_transcode_output(token, dest.clone()).expect("commit");

        assert_eq!(saved_path, path_to_string(&dest));
        assert!(dest.exists(), "destination file should exist after commit");
        assert!(!source.exists(), "source temp should be moved away");
        clear_pending_for_test();
    }

    #[test]
    #[serial]
    fn commit_token_discard_removes_file() {
        clear_pending_for_test();
        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("source.mp4");
        fs::write(&source, b"video").expect("write source");

        let token = register_transcode_commit(source.clone()).expect("register token");
        discard_transcode_output(token).expect("discard");

        assert!(!source.exists(), "source temp should be removed on discard");
        clear_pending_for_test();
    }

    #[test]
    #[serial]
    fn commit_token_unknown_returns_error() {
        clear_pending_for_test();
        let dir = tempfile::tempdir().expect("tempdir");
        let dest = dir.path().join("dest.mp4");
        let error =
            commit_transcode_output("missing-token".to_string(), dest).expect_err("should fail");
        assert!(
            error.to_string().contains("Unknown commitToken"),
            "unexpected error: {}",
            error
        );
        clear_pending_for_test();
    }

    #[test]
    #[serial]
    fn cleanup_pending_transcodes_removes_expired_entries() {
        clear_pending_for_test();
        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("source.mp4");
        fs::write(&source, b"video").expect("write source");

        let token = register_transcode_commit(source.clone()).expect("register token");
        {
            let mut guard = PENDING_TRANSCODES.lock();
            let entry = guard.get_mut(&token).expect("pending token entry");
            entry.created_at = SystemTime::now()
                .checked_sub(Duration::from_secs(120))
                .expect("system time subtraction");
        }

        cleanup_pending_transcodes(Duration::from_secs(1));

        {
            let guard = PENDING_TRANSCODES.lock();
            assert!(
                !guard.contains_key(&token),
                "expired token should be removed"
            );
        }
        assert!(
            !source.exists(),
            "expired token cleanup should remove temporary file"
        );
        clear_pending_for_test();
    }
}
