use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use tauri::Emitter;

use crate::error::AppError;
use super::discovery::get_ffmpeg_path;
use super::progress::parse_ffmpeg_progress;

/// Minimum interval between progress emits to reduce IPC and React re-renders.
const PROGRESS_EMIT_INTERVAL: Duration = Duration::from_millis(150);

static ACTIVE_PROCESSES: Mutex<Option<Child>> = Mutex::new(None);

fn read_stream<R: std::io::Read + Send + 'static>(
    reader: R,
    collect_stderr: Option<Arc<Mutex<String>>>,
    duration: Arc<Mutex<Option<f64>>>,
    app: Option<tauri::AppHandle>,
    window_label: Option<String>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut current_duration = *duration.lock().unwrap_or_else(|e| e.into_inner());
        let mut last_emit = Instant::now();
        let mut last_progress = 0.0_f64;
        let stream_reader = BufReader::new(reader);
        for line in stream_reader.lines().filter_map(Result::ok) {
            if let Some(ref buf) = collect_stderr {
                if let Ok(mut guard) = buf.lock() {
                    guard.push_str(&line);
                    guard.push('\n');
                }
            }
            let (progress, d) = parse_ffmpeg_progress(&line, current_duration);
            if d.is_some() {
                current_duration = d;
                if let Ok(mut guard) = duration.lock() {
                    *guard = d;
                }
            }
            if let (Some(p), Some(handle)) = (progress, app.as_ref()) {
                let now = Instant::now();
                let should_emit = now.duration_since(last_emit) >= PROGRESS_EMIT_INTERVAL
                    || (p - last_progress).abs() >= 0.01
                    || p >= 1.0;
                if should_emit {
                    last_emit = now;
                    last_progress = p;
                    let _ = if let Some(ref lbl) = window_label {
                        handle.emit_to(lbl, "ffmpeg-progress", p)
                    } else {
                        handle.emit("ffmpeg-progress", p)
                    };
                }
            }
        }
    })
}

/// Run FFmpeg and block until completion. Used when we need to wait (e.g. preview, transcode).
/// Optionally emit progress events to the frontend via app and window_label.
pub fn run_ffmpeg_blocking(
    args: Vec<String>,
    app: Option<&tauri::AppHandle>,
    window_label: Option<&str>,
) -> Result<(), AppError> {
    let ffmpeg_path = get_ffmpeg_path()?;
    let path_str = ffmpeg_path.to_string_lossy();

    let mut child = Command::new(&*path_str)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn FFmpeg: {}", e))?;

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

    {
        let mut guard = ACTIVE_PROCESSES.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(child);
    }

    let duration: Arc<Mutex<Option<f64>>> = Arc::new(Mutex::new(None));
    let stderr_buffer = Arc::new(Mutex::new(String::new()));
    let app_stdout = app.cloned();
    let app_stderr = app.cloned();
    let label = window_label.map(String::from);

    let stdout_handle = read_stream(
        stdout,
        None,
        Arc::clone(&duration),
        app_stdout,
        label.clone(),
    );
    let stderr_handle = read_stream(
        stderr,
        Some(Arc::clone(&stderr_buffer)),
        Arc::clone(&duration),
        app_stderr,
        label,
    );

    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    let mut guard = ACTIVE_PROCESSES.lock().unwrap_or_else(|e| e.into_inner());
    let child = guard.take();
    drop(guard);

    let status = match child {
        Some(mut c) => c.wait().map_err(|e| e.to_string())?,
        None => return Err(AppError::Aborted),
    };

    let stderr_str = stderr_buffer
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();

    if status.success() {
        Ok(())
    } else {
        let code = status.code().unwrap_or(-1);
        Err(AppError::FfmpegFailed {
            code,
            stderr: stderr_str,
        })
    }
}

pub fn terminate_all_ffmpeg() {
    let mut guard = ACTIVE_PROCESSES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}
