mod error;
pub mod ffmpeg;
mod log_plugin;

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::{Emitter, Manager};

#[derive(Default)]
pub(crate) struct AppState {
    pending_opened_files: Arc<Mutex<Vec<PathBuf>>>,
}

#[cfg(test)]
impl AppState {
    pub fn with_pending(paths: Vec<PathBuf>) -> Self {
        Self {
            pending_opened_files: Arc::new(Mutex::new(paths)),
        }
    }
}

use error::AppError;
use ffmpeg::{
    build_ffmpeg_command, cleanup_previous_preview_paths, cleanup_transcode_temp,
    format_args_for_display_multiline, get_cached_extract, parse_ffmpeg_error, run_ffmpeg_blocking,
    set_cached_extract, set_transcode_temp, store_preview_paths_for_cleanup, terminate_all_ffmpeg,
    TempFileManager, TranscodeOptions,
};
use ffmpeg::ffprobe::get_video_metadata_impl;
use ffmpeg::FfmpegErrorPayload;

fn build_error_payload(e: &AppError) -> FfmpegErrorPayload {
    match e {
        AppError::FfmpegFailed { code, stderr } => parse_ffmpeg_error(stderr, Some(*code)),
        _ => parse_ffmpeg_error(&e.to_string(), None),
    }
}

async fn run_ffmpeg_step(
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
                target: "tiny_vid::commands",
                "emitting ffmpeg-complete to window={}",
                window_label_owned
            );
            let _ = app.emit_to(&window_label_owned, "ffmpeg-complete", ());
            Ok(())
        }
        Ok(Err(e)) => {
            log::error!(
                target: "tiny_vid::commands",
                "ffmpeg-error: {}",
                e
            );
            let payload = build_error_payload(&e);
            let _ = app.emit_to(&window_label_owned, "ffmpeg-error", payload);
            Err(e)
        }
        Err(join_err) => {
            let e = AppError::from(join_err.to_string());
            log::error!(
                target: "tiny_vid::commands",
                "ffmpeg-error (join): {}",
                e
            );
            let payload = build_error_payload(&e);
            let _ = app.emit_to(&window_label_owned, "ffmpeg-error", payload);
            Err(e)
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
async fn ffmpeg_transcode_to_temp(
    input_path: PathBuf,
    options: TranscodeOptions,
    app: tauri::AppHandle,
    window: tauri::Window,
) -> Result<String, AppError> {
    log::info!(
        target: "tiny_vid::commands",
        "ffmpeg_transcode_to_temp: input={}",
        input_path.display()
    );
    cleanup_transcode_temp();

    let ext = options
        .output_format
        .as_deref()
        .unwrap_or("mp4")
        .to_lowercase();
    let suffix = format!("transcode-output.{}", ext);

    let temp = TempFileManager::default();
    let output_path = temp
        .create(&suffix, None)
        .map_err(AppError::from)?;
    let output_str = output_path.to_string_lossy().to_string();

    set_transcode_temp(Some(output_path.clone()));

    let args = build_ffmpeg_command(&input_path.to_string_lossy(), &output_str, &options)?;
    let duration_secs = options.duration_secs;

    match run_ffmpeg_step(args, &app, window.label(), duration_secs).await {
        Ok(()) => {
            log::info!(
                target: "tiny_vid::commands",
                "ffmpeg_transcode_to_temp: complete -> {}",
                output_str
            );
            Ok(output_str)
        }
        Err(e) => {
            cleanup_transcode_temp();
            Err(e)
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
fn move_compressed_file(source: PathBuf, dest: PathBuf) -> Result<(), AppError> {
    log::info!(
        target: "tiny_vid::commands",
        "move_compressed_file: {} -> {}",
        source.display(),
        dest.display()
    );
    match fs::rename(&source, &dest) {
        Ok(()) => {
            log::debug!(target: "tiny_vid::commands", "move_compressed_file: complete");
            Ok(())
        }
        Err(e) => {
            #[cfg(unix)]
            if e.raw_os_error() == Some(18) {
                // EXDEV: cross-device link
                fs::copy(&source, &dest)?;
                fs::remove_file(&source)?;
                return Ok(());
            }
            #[cfg(windows)]
            if e.raw_os_error() == Some(17) {
                // ERROR_NOT_SAME_DEVICE
                fs::copy(&source, &dest)?;
                fs::remove_file(&source)?;
                return Ok(());
            }
            Err(e.into())
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
fn cleanup_temp_file(path: PathBuf) -> Result<(), AppError> {
    log::info!(
        target: "tiny_vid::commands",
        "cleanup_temp_file: path={}",
        path.display()
    );
    let _ = fs::remove_file(&path);
    cleanup_transcode_temp();
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
async fn ffmpeg_preview(
    input_path: PathBuf,
    options: TranscodeOptions,
    app: tauri::AppHandle,
    window: tauri::Window,
) -> Result<PreviewResult, AppError> {
    let input_str = input_path.to_string_lossy().to_string();
    let preview_duration_u32 = options.preview_duration.unwrap_or(3);
    let preview_duration = preview_duration_u32 as f64;

    log::info!(
        target: "tiny_vid::commands",
        "ffmpeg_preview: input={}",
        input_path.display()
    );
    cleanup_previous_preview_paths(&input_str, preview_duration_u32);

    let ext = options
        .output_format
        .as_deref()
        .unwrap_or("mp4")
        .to_lowercase();
    let preview_suffix = format!("preview-output.{}", ext);

    let temp = TempFileManager::default();
    let output_path = temp
        .create(&preview_suffix, None)
        .map_err(AppError::from)?;

    let original_path = match get_cached_extract(&input_str, preview_duration_u32) {
        Some(cached) => {
            log::info!(
                target: "tiny_vid::commands",
                "ffmpeg_preview: cache hit, reusing extracted segment"
            );
            cached
        }
        None => {
            let path = temp.create("preview-original.mp4", None).map_err(AppError::from)?;

            let extract_args = vec![
                "-nostdin".to_string(),
                "-threads".to_string(),
                "0".to_string(),
                "-thread_queue_size".to_string(),
                "512".to_string(),
                "-ss".to_string(),
                "0".to_string(),
                "-t".to_string(),
                preview_duration.to_string(),
                "-i".to_string(),
                input_str.clone(),
                "-c".to_string(),
                "copy".to_string(),
                path.to_string_lossy().to_string(),
            ];

            run_ffmpeg_step(extract_args, &app, window.label(), None).await?;
            set_cached_extract(input_str.clone(), preview_duration_u32, path.clone());
            path
        }
    };

    let transcode_args = build_ffmpeg_command(
        &original_path.to_string_lossy(),
        &output_path.to_string_lossy(),
        &options,
    )?;

    run_ffmpeg_step(transcode_args, &app, window.label(), None).await?;

    let input_size = fs::metadata(&input_path)?.len();
    let compressed_size = fs::metadata(&output_path)?.len();
    let original_size = fs::metadata(&original_path)?.len();
    let ratio = compressed_size as f64 / original_size as f64;
    let estimated_size = (input_size as f64 * ratio) as u64;

    store_preview_paths_for_cleanup(original_path.clone(), output_path.clone());

    log::info!(
        target: "tiny_vid::commands",
        "ffmpeg_preview: complete, estimated_size={}",
        estimated_size
    );
    Ok(PreviewResult {
        original_path: original_path.to_string_lossy().to_string(),
        compressed_path: output_path.to_string_lossy().to_string(),
        estimated_size,
    })
}

#[tauri::command(rename_all = "camelCase")]
fn get_file_size(path: PathBuf) -> Result<u64, AppError> {
    log::debug!(
        target: "tiny_vid::commands",
        "get_file_size: path={}",
        path.display()
    );
    fs::metadata(&path).map(|m| m.len()).map_err(Into::into)
}

#[tauri::command(rename_all = "camelCase")]
fn get_video_metadata(path: PathBuf) -> Result<VideoMetadataResult, AppError> {
    log::debug!(
        target: "tiny_vid::commands",
        "get_video_metadata: path={}",
        path.display()
    );
    let meta = get_video_metadata_impl(&path)?;
    Ok(VideoMetadataResult {
        duration: meta.duration,
        width: meta.width,
        height: meta.height,
        size: meta.size,
        size_mb: meta.size as f64 / 1024.0 / 1024.0,
    })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct VideoMetadataResult {
    duration: f64,
    width: u32,
    height: u32,
    size: u64,
    size_mb: f64,
}

#[tauri::command(rename_all = "camelCase")]
fn preview_ffmpeg_command(options: TranscodeOptions, input_path: Option<String>) -> String {
    let input_str = input_path.as_deref().unwrap_or("<input>");
    let output_str = "<output>";
    let args = build_ffmpeg_command(input_str, output_str, &options)
        .unwrap_or_else(|e| vec!["# error".into(), e.to_string()]);
    format!("ffmpeg\n{}", format_args_for_display_multiline(&args))
}

#[tauri::command]
fn ffmpeg_terminate() {
    log::info!(target: "tiny_vid::commands", "ffmpeg_terminate: terminating all FFmpeg processes");
    terminate_all_ffmpeg();
}

#[tauri::command(rename_all = "camelCase")]
fn get_pending_opened_files(state: tauri::State<'_, AppState>) -> Vec<String> {
    let mut files = state.pending_opened_files.lock().unwrap();
    files
        .drain(..)
        .map(|p| p.to_string_lossy().to_string())
        .collect()
}

fn buffer_opened_files(app: &tauri::AppHandle, files: Vec<PathBuf>) {
    if files.is_empty() {
        return;
    }
    let asset_scope = app.asset_protocol_scope();
    for file in &files {
        let _ = asset_scope.allow_file(file);
    }
    let paths: Vec<String> = files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    {
        let state = app.state::<AppState>();
        let mut pending = state.pending_opened_files.lock().unwrap();
        pending.extend(files);
    }
    let _ = app.emit("open-file", paths);
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodecInfo {
    pub value: String,
    pub name: String,
    pub formats: Vec<String>,
    pub supports_tune: bool,
    pub preset_type: String,
}

#[derive(serde::Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BuildVariantResult {
    pub variant: &'static str,
    pub codecs: Vec<CodecInfo>,
}

/// Return CodecInfo for a known codec string. Panics on unknown codec.
fn get_codec_info(codec: &str) -> CodecInfo {
    match codec {
        "libx264" => CodecInfo {
            value: "libx264".to_string(),
            name: "H.264 (Widest support)".to_string(),
            formats: vec!["mp4".to_string()],
            supports_tune: true,
            preset_type: "x264".to_string(),
        },
        "libx265" => CodecInfo {
            value: "libx265".to_string(),
            name: "H.265 (Smaller files)".to_string(),
            formats: vec!["mp4".to_string()],
            supports_tune: false,
            preset_type: "x265".to_string(),
        },
        "libsvtav1" => CodecInfo {
            value: "libsvtav1".to_string(),
            name: "AV1 (Smallest files)".to_string(),
            formats: vec!["mp4".to_string(), "webm".to_string()],
            supports_tune: false,
            preset_type: "av1".to_string(),
        },
        "libvpx-vp9" => CodecInfo {
            value: "libvpx-vp9".to_string(),
            name: "VP9 (Browser-friendly WebM)".to_string(),
            formats: vec!["webm".to_string()],
            supports_tune: false,
            preset_type: "vp9".to_string(),
        },
        "h264_videotoolbox" => CodecInfo {
            value: "h264_videotoolbox".to_string(),
            name: "H.264 (VideoToolbox)".to_string(),
            formats: vec!["mp4".to_string()],
            supports_tune: false,
            preset_type: "vt".to_string(),
        },
        "hevc_videotoolbox" => CodecInfo {
            value: "hevc_videotoolbox".to_string(),
            name: "H.265 (VideoToolbox)".to_string(),
            formats: vec!["mp4".to_string()],
            supports_tune: false,
            preset_type: "vt".to_string(),
        },
        _ => panic!("Unknown codec: {}", codec),
    }
}

/// When non-LGPL (software) codecs are available, filter out VideoToolbox so we prefer libx264/etc.
fn filter_codecs_for_display(available: &[String]) -> Vec<String> {
    const NON_VT: &[&str] = &["libx264", "libx265", "libsvtav1", "libvpx-vp9"];
    const VT: &[&str] = &["h264_videotoolbox", "hevc_videotoolbox"];
    let has_non_vt = available.iter().any(|c| NON_VT.contains(&c.as_str()));
    if has_non_vt {
        available.iter()
            .filter(|c| !VT.contains(&c.as_str()))
            .cloned()
            .collect()
    } else {
        available.to_vec()
    }
}

#[tauri::command(rename_all = "camelCase")]
fn get_build_variant() -> Result<BuildVariantResult, error::AppError> {
    let available = ffmpeg::discovery::get_available_codecs()?;
    let codecs = filter_codecs_for_display(&available);

    if codecs.is_empty() {
        return Err(error::AppError::from(
            "No supported video codecs found in FFmpeg. Please ensure FFmpeg is properly installed with codec support."
        ));
    }

    #[cfg(feature = "lgpl-macos")]
    let variant = "lgpl-macos";
    #[cfg(not(feature = "lgpl-macos"))]
    let variant = "standalone";
    
    Ok(BuildVariantResult {
        variant,
        codecs: codecs.iter().map(|s| get_codec_info(s)).collect(),
    })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewResult {
    original_path: String,
    compressed_path: String,
    estimated_size: u64,
}

#[cfg(test)]
mod build_variant_tests {
    use super::*;
    
    #[test]
    fn codec_info_has_correct_metadata() {
        let info = get_codec_info("libx264");
        assert_eq!(info.value, "libx264");
        assert_eq!(info.name, "H.264 (Widest support)");
        assert_eq!(info.formats, vec!["mp4"]);
        assert!(info.supports_tune);
        assert_eq!(info.preset_type, "x264");
    }
    
    #[test]
    fn all_codecs_have_info() {
        for codec in ["libx264", "libx265", "libsvtav1", "libvpx-vp9", 
                      "h264_videotoolbox", "hevc_videotoolbox"] {
            let info = get_codec_info(codec);
            assert!(!info.value.is_empty());
            assert!(!info.name.is_empty());
        }
    }

    #[test]
    fn get_codec_info_returns_correct_formats() {
        let x264 = get_codec_info("libx264");
        assert_eq!(x264.formats, vec!["mp4"]);

        let av1 = get_codec_info("libsvtav1");
        assert_eq!(av1.formats, vec!["mp4", "webm"]);

        let vp9 = get_codec_info("libvpx-vp9");
        assert_eq!(vp9.formats, vec!["webm"]);
    }

    #[test]
    fn get_codec_info_preset_types() {
        assert_eq!(get_codec_info("libx264").preset_type, "x264");
        assert_eq!(get_codec_info("libx265").preset_type, "x265");
        assert_eq!(get_codec_info("libsvtav1").preset_type, "av1");
        assert_eq!(get_codec_info("h264_videotoolbox").preset_type, "vt");
    }

    #[test]
    fn filter_codecs_hides_videotoolbox_when_non_vt_available() {
        // When libx264 (or any non-VT) is available, VideoToolbox is filtered out
        let available = vec![
            "libx264".to_string(),
            "h264_videotoolbox".to_string(),
            "hevc_videotoolbox".to_string(),
        ];
        let filtered = filter_codecs_for_display(&available);
        assert_eq!(filtered, vec!["libx264"]);
    }

    #[test]
    fn filter_codecs_keeps_videotoolbox_when_only_vt_available() {
        let available = vec![
            "h264_videotoolbox".to_string(),
            "hevc_videotoolbox".to_string(),
        ];
        let filtered = filter_codecs_for_display(&available);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&"h264_videotoolbox".to_string()));
        assert!(filtered.contains(&"hevc_videotoolbox".to_string()));
    }
    
    #[test]
    #[ignore = "requires FFmpeg on system to detect codecs; run with: cargo test get_build_variant_returns_valid_codecs -- --ignored"]
    fn get_build_variant_returns_valid_codecs() {
        let result = get_build_variant();
        assert!(result.is_ok(), "Should detect codecs: {:?}", result.err());
        let variant = result.unwrap();
        assert!(!variant.codecs.is_empty(), "Should have at least one codec");
        
        #[cfg(feature = "lgpl-macos")]
        assert_eq!(variant.variant, "lgpl-macos");

        #[cfg(not(feature = "lgpl-macos"))]
        assert_eq!(variant.variant, "standalone");

        for codec in &variant.codecs {
            assert!(
                matches!(codec.value.as_str(), 
                    "libx264" | "libx265" | "libsvtav1" | "libvpx-vp9" | 
                    "h264_videotoolbox" | "hevc_videotoolbox"
                ),
                "Unexpected codec: {}",
                codec.value
            );
        }
    }
}

#[cfg(test)]
mod test_util;

#[cfg(test)]
mod commands_tests;

#[cfg(test)]
mod integration_tests;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(log_plugin::build_log_plugin().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .manage(AppState::default())
        .setup(|app| {
            #[cfg(any(windows, target_os = "linux"))]
            {
                let mut files = Vec::new();
                for maybe_file in std::env::args().skip(1) {
                    if maybe_file.starts_with('-') {
                        continue;
                    }
                    if let Ok(url) = url::Url::parse(&maybe_file) {
                        if let Ok(path) = url.to_file_path() {
                            files.push(path);
                        }
                    } else {
                        files.push(PathBuf::from(maybe_file));
                    }
                }
                if !files.is_empty() {
                    buffer_opened_files(&app.handle().clone(), files);
                }
            }

            use tauri::menu::{AboutMetadata, MenuBuilder, PredefinedMenuItem, SubmenuBuilder};
            let pkg = app.package_info();

            let about = PredefinedMenuItem::about(
                app,
                None,
                Some(AboutMetadata {
                    name: Some(pkg.name.clone()),
                    version: Some(pkg.version.to_string()),
                    copyright: Some("Copyright © 2025 Mantas Mikalauskis".into()),
                    credits: Some("Compress and optimize video files with H.264, H.265, and AV1.".into()),
                    ..Default::default()
                }),
            )?;
            let quit = PredefinedMenuItem::quit(app, None)?;
            let app_menu = SubmenuBuilder::new(app, &pkg.name)
                .item(&about)
                .separator()
                .item(&quit)
                .build()?;

            let file_menu = SubmenuBuilder::new(app, "File")
                .text("open-file", "Open File")
                .build()?;

            let fullscreen = PredefinedMenuItem::fullscreen(app, None)?;
            let view_menu = SubmenuBuilder::new(app, "View")
                .item(&fullscreen)
                .build()?;

            let minimize = PredefinedMenuItem::minimize(app, None)?;
            let maximize = PredefinedMenuItem::maximize(app, None)?;
            let close_window = PredefinedMenuItem::close_window(app, None)?;
            let show_all = PredefinedMenuItem::show_all(app, None)?;
            let window_menu = SubmenuBuilder::new(app, "Window")
                .item(&minimize)
                .item(&maximize)
                .item(&close_window)
                .separator()
                .item(&show_all)
                .build()?;

            let menu = MenuBuilder::new(app)
                .items(&[&app_menu, &file_menu, &view_menu, &window_menu])
                .build()?;
            app.set_menu(menu)?;

            app.on_menu_event(move |_app, event| {
                if event.id().0.as_str() == "open-file" {
                    let _ = _app.emit("menu-open-file", ());
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ffmpeg_transcode_to_temp,
            ffmpeg_preview,
            preview_ffmpeg_command,
            ffmpeg_terminate,
            get_file_size,
            get_video_metadata,
            get_build_variant,
            move_compressed_file,
            cleanup_temp_file,
            get_pending_opened_files,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app, event| {
        match &event {
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            tauri::RunEvent::Opened { urls } => {
                let files: Vec<PathBuf> = urls
                    .iter()
                    .filter_map(|u| u.to_file_path().ok())
                    .collect();
                if !files.is_empty() {
                    buffer_opened_files(app, files);
                }
            }
            tauri::RunEvent::ExitRequested { .. } => {
                log::info!(target: "tiny_vid::commands", "app exit requested, cleaning up");
                cleanup_transcode_temp();
            }
            _ => {}
        }
    });
}
