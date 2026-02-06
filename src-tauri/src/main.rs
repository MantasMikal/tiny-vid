// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

const STARTUP_CLEANUP_MAX_AGE_HOURS: u64 = 24;

fn main() {
    let _ = fix_path_env::fix();
    let max_age = std::time::Duration::from_secs(STARTUP_CLEANUP_MAX_AGE_HOURS * 60 * 60);
    tiny_vid_tauri_lib::ffmpeg::cleanup_old_temp_files(max_age);
    tiny_vid_tauri_lib::run()
}
