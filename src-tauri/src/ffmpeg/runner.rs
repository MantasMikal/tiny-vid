//! FFmpeg process spawning and progress parsing.
//!
//! Spawns FFmpeg as a child process, parses progress from stdout (pipe:1),
//! and optionally emits progress events to the frontend. Uses a background
//! thread to read the progress stream while the main thread waits for completion.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tauri::Emitter;

use crate::error::AppError;
use super::discovery::get_ffmpeg_path;
use super::progress::parse_ffmpeg_progress;

/// Sentinel for "duration not yet known". AtomicU64 cannot hold Option<f64>,
/// so we encode duration as f64 bits; u64::MAX means "not yet known".
const NONE_DURATION_BITS: u64 = u64::MAX;

/// Minimum interval between progress emits to reduce IPC and React re-renders.
const PROGRESS_EMIT_INTERVAL: Duration = Duration::from_millis(150);
/// Keep only the last N bytes of stderr to avoid unbounded memory growth.
const MAX_STDERR_BYTES: usize = 64 * 1024;

/// Single active FFmpeg process. Only one transcode/preview at a time.
static ACTIVE_FFMPEG_PROCESS: Mutex<Option<Child>> = Mutex::new(None);

/// Configuration for FFmpeg output stream reading (stdout or stderr).
struct ReadStreamConfig {
    collect_stderr: Option<Arc<Mutex<Vec<u8>>>>,
    duration: Arc<AtomicU64>,
    app: Option<tauri::AppHandle>,
    window_label: Option<String>,
    progress_collector: Option<Arc<Mutex<Vec<f64>>>>,
}

fn read_stream<R: std::io::Read + Send + 'static>(
    reader: R,
    config: ReadStreamConfig,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let load_duration = || {
            let bits = config.duration.load(Ordering::Relaxed);
            if bits == NONE_DURATION_BITS {
                None
            } else {
                Some(f64::from_bits(bits))
            }
        };
        let mut current_duration = load_duration();
        let mut last_emit = Instant::now();
        let mut last_progress = 0.0_f64;
        let mut stream_reader = BufReader::new(reader);
        let mut line_buf = Vec::with_capacity(256);
        while stream_reader.read_until(b'\n', &mut line_buf).unwrap_or(0) > 0 {
            let line = std::str::from_utf8(&line_buf)
                .unwrap_or("")
                .trim_end_matches(['\n', '\r']);
            if let Some(ref buf) = config.collect_stderr {
                let mut guard = buf.lock();
                guard.extend_from_slice(line.as_bytes());
                guard.push(b'\n');
                if guard.len() > MAX_STDERR_BYTES {
                    let excess = guard.len() - MAX_STDERR_BYTES;
                    guard.drain(..excess);
                }
            }
            let (progress, d) = parse_ffmpeg_progress(line, current_duration);
            if let Some(new_dur) = d {
                current_duration = Some(new_dur);
                config.duration.store(new_dur.to_bits(), Ordering::Relaxed);
            }
            if let Some(p) = progress {
                if let Some(ref collector) = config.progress_collector {
                    let mut guard = collector.lock();
                    guard.push(p);
                }
                if let Some(handle) = config.app.as_ref() {
                    let now = Instant::now();
                    let should_emit = now.duration_since(last_emit) >= PROGRESS_EMIT_INTERVAL
                        || (p - last_progress).abs() >= 0.01
                        || p >= 1.0;
                    if should_emit {
                        last_emit = now;
                        last_progress = p;
                        let _ = if let Some(ref lbl) = config.window_label {
                            handle.emit_to(lbl, "ffmpeg-progress", p)
                        } else {
                            handle.emit("ffmpeg-progress", p)
                        };
                    }
                }
            }
            line_buf.clear();
        }
    })
}

/// Run FFmpeg and block until completion. Used when we need to wait (e.g. preview, transcode).
/// Optionally emit progress events to the frontend via app and window_label.
/// duration_secs: if provided, initializes the shared duration so progress can be computed
/// immediately from out_time_ms (avoids race with Duration line on stderr).
/// progress_collector: when provided (e.g. in tests), collects all progress values for verification.
pub fn run_ffmpeg_blocking(
    args: Vec<String>,
    app: Option<&tauri::AppHandle>,
    window_label: Option<&str>,
    duration_secs: Option<f64>,
    progress_collector: Option<Arc<Mutex<Vec<f64>>>>,
) -> Result<(), AppError> {
    let ffmpeg_path = get_ffmpeg_path()?;
    let path_str = ffmpeg_path.to_string_lossy();

    let input_arg = args.iter().position(|a| a == "-i").and_then(|i| args.get(i + 1));
    let output_arg = args.last();
    log::debug!(
        target: "tiny_vid::ffmpeg::runner",
        "Spawning FFmpeg: path={}, input={:?}, output={:?}",
        path_str,
        input_arg,
        output_arg
    );

    let mut child = Command::new(&*path_str)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn FFmpeg: {}", e))?;

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

    {
        let mut guard = ACTIVE_FFMPEG_PROCESS.lock();
        *guard = Some(child);
    }

    let duration = Arc::new(AtomicU64::new(
        duration_secs
            .filter(|&d| d > 0.0)
            .map(f64::to_bits)
            .unwrap_or(NONE_DURATION_BITS),
    ));
    let stderr_buffer = Arc::new(Mutex::new(Vec::new()));
    let app_stdout = app.cloned();
    let app_stderr = app.cloned();
    let label = window_label.map(String::from);

    let stdout_handle = read_stream(
        stdout,
        ReadStreamConfig {
            collect_stderr: None,
            duration: Arc::clone(&duration),
            app: app_stdout,
            window_label: label.clone(),
            progress_collector,
        },
    );
    let stderr_handle = read_stream(
        stderr,
        ReadStreamConfig {
            collect_stderr: Some(Arc::clone(&stderr_buffer)),
            duration: Arc::clone(&duration),
            app: app_stderr,
            window_label: label,
            progress_collector: None,
        },
    );

    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    let mut guard = ACTIVE_FFMPEG_PROCESS.lock();
    let child = guard.take();
    drop(guard);

    let status = match child {
        Some(mut c) => c.wait().map_err(|e| e.to_string())?,
        None => {
            log::warn!(
                target: "tiny_vid::ffmpeg::runner",
                "FFmpeg process was aborted (terminated externally)"
            );
            return Err(AppError::aborted());
        }
    };

    let stderr_bytes = stderr_buffer.lock().clone();
    let stderr_str = String::from_utf8_lossy(&stderr_bytes).to_string();

    if status.success() {
        log::info!(
            target: "tiny_vid::ffmpeg::runner",
            "FFmpeg completed successfully"
        );
        Ok(())
    } else {
        let code = status.code().unwrap_or(-1);
        let err_preview = stderr_str
            .lines()
            .rev()
            .take(3)
            .collect::<Vec<_>>()
            .join("; ");
        log::error!(
            target: "tiny_vid::ffmpeg::runner",
            "FFmpeg failed (code={}): {}",
            code,
            err_preview
        );
        Err(AppError::FfmpegFailed {
            code,
            stderr: stderr_str,
        })
    }
}

pub fn terminate_all_ffmpeg() {
    let mut guard = ACTIVE_FFMPEG_PROCESS.lock();
    if let Some(mut child) = guard.take() {
        log::info!(
            target: "tiny_vid::ffmpeg::runner",
            "Terminating FFmpeg process"
        );
        let _ = child.kill();
        let _ = child.wait();
    }
}
