mod codec;
mod commands;
mod error;
pub mod ffmpeg;
mod log_plugin;
mod preview;

use std::path::PathBuf;

use tauri::Emitter;

fn setup_menu(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{AboutMetadata, MenuBuilder, PredefinedMenuItem, SubmenuBuilder};
    let pkg = app.package_info();

    let about = PredefinedMenuItem::about(
        app,
        None,
        Some(AboutMetadata {
            name: Some(pkg.name.clone()),
            version: Some(pkg.version.to_string()),
            copyright: Some("Copyright © 2025 Mantas Mikalauskis".into()),
            credits: Some(
                "Compress and optimize video files with H.264, H.265, and AV1.".into(),
            ),
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
}

#[derive(Default)]
pub(crate) struct AppState {
    pending_opened_files: std::sync::Arc<parking_lot::Mutex<Vec<PathBuf>>>,
}

#[cfg(test)]
impl AppState {
    pub fn with_pending(paths: Vec<PathBuf>) -> Self {
        Self {
            pending_opened_files: std::sync::Arc::new(parking_lot::Mutex::new(paths)),
        }
    }
}

pub use codec::CodecInfo;

#[cfg(test)]
mod test_util;

#[cfg(test)]
mod commands_tests;

#[cfg(test)]
mod integration_tests;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use ffmpeg::{cleanup_preview_transcode_cache, cleanup_transcode_temp};

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
                    commands::buffer_opened_files(&app.handle().clone(), files);
                }
            }

            setup_menu(app).map_err(Into::into)
        })
        .invoke_handler(tauri::generate_handler![
            commands::ffmpeg_transcode_to_temp,
            commands::ffmpeg_preview,
            commands::ffmpeg_preview_estimate,
            commands::preview_ffmpeg_command,
            commands::ffmpeg_terminate,
            commands::get_file_size,
            commands::get_video_metadata,
            commands::get_build_variant,
            commands::move_compressed_file,
            commands::cleanup_temp_file,
            commands::get_pending_opened_files,
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
                    commands::buffer_opened_files(app, files);
                }
            }
            tauri::RunEvent::ExitRequested { .. } => {
                log::info!(target: "tiny_vid::commands", "app exit requested, cleaning up");
                cleanup_transcode_temp();
                cleanup_preview_transcode_cache();
            }
            _ => {}
        }
    });
}
