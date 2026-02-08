use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::{Value, json};
use tiny_vid_core::ffmpeg::{FfmpegProgressPayload, TranscodeOptions};
use tiny_vid_core::sidecar_api;

const STARTUP_CLEANUP_MAX_AGE_HOURS: u64 = 24;

#[derive(Debug, Deserialize)]
struct RpcRequest {
    id: u64,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, serde::Serialize)]
struct RpcSuccess {
    id: u64,
    result: Value,
}

#[derive(Debug, serde::Serialize)]
struct RpcFailure {
    id: u64,
    error: RpcErrorPayload,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RpcErrorPayload {
    summary: String,
    detail: String,
}

#[derive(Debug, serde::Serialize)]
struct RpcEvent {
    event: String,
    payload: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
enum JobKind {
    Preview,
    Transcode,
}

#[derive(Debug, Clone, Copy)]
struct ActiveJob {
    job_id: u64,
    kind: JobKind,
}

#[derive(Clone)]
struct JobState {
    active_job: Arc<Mutex<Option<ActiveJob>>>,
    next_job_id: Arc<AtomicU64>,
}

impl JobState {
    fn new() -> Self {
        Self {
            active_job: Arc::new(Mutex::new(None)),
            next_job_id: Arc::new(AtomicU64::new(1)),
        }
    }

    fn begin_job(&self, kind: JobKind) -> Result<ActiveJob, tiny_vid_core::error::AppError> {
        let mut guard = self.active_job.lock();
        if let Some(existing) = *guard {
            return Err(tiny_vid_core::error::AppError::from(format!(
                "Another job is already running (jobId={}, kind={:?})",
                existing.job_id, existing.kind
            )));
        }
        let job = ActiveJob {
            job_id: self.next_job_id.fetch_add(1, Ordering::Relaxed),
            kind,
        };
        *guard = Some(job);
        Ok(job)
    }

    fn finish_job(&self, job_id: u64) {
        let mut guard = self.active_job.lock();
        if let Some(active) = *guard
            && active.job_id == job_id
        {
            *guard = None;
        }
    }

    fn current_job(&self) -> Option<ActiveJob> {
        *self.active_job.lock()
    }
}

struct ActiveJobGuard {
    state: JobState,
    job_id: u64,
}

impl ActiveJobGuard {
    fn new(state: JobState, job_id: u64) -> Self {
        Self { state, job_id }
    }
}

impl Drop for ActiveJobGuard {
    fn drop(&mut self) {
        self.state.finish_job(self.job_id);
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
enum MediaInspectParams {
    Metadata {
        #[serde(rename = "inputPath")]
        input_path: PathBuf,
    },
    CommandPreview {
        options: TranscodeOptions,
        #[serde(rename = "inputPath")]
        input_path: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
enum MediaProcessParams {
    Preview {
        #[serde(rename = "inputPath")]
        input_path: PathBuf,
        options: TranscodeOptions,
        #[serde(rename = "previewStartSeconds")]
        preview_start_seconds: Option<f64>,
        #[serde(rename = "includeEstimate", default = "default_true")]
        include_estimate: bool,
    },
    Transcode {
        #[serde(rename = "inputPath")]
        input_path: PathBuf,
        options: TranscodeOptions,
    },
    Commit {
        #[serde(rename = "commitToken")]
        commit_token: String,
        #[serde(rename = "outputPath")]
        output_path: PathBuf,
    },
    Discard {
        #[serde(rename = "commitToken")]
        commit_token: String,
    },
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct MediaCancelParams {
    job_id: Option<u64>,
}

fn default_true() -> bool {
    true
}

type SharedWriter = Arc<Mutex<io::Stdout>>;

fn write_json_line<T: serde::Serialize>(writer: &mut impl Write, value: &T) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, value)
        .map_err(|e| io::Error::other(format!("serialize response: {}", e)))?;
    writer.write_all(b"\n")?;
    writer.flush()
}

fn write_json_line_shared<T: serde::Serialize>(writer: &SharedWriter, value: &T) -> io::Result<()> {
    let mut guard = writer.lock();
    write_json_line(&mut *guard, value)
}

fn emit_rpc_event(writer: &SharedWriter, event: &str, payload: Value) {
    let message = RpcEvent {
        event: event.to_string(),
        payload,
    };
    let _ = write_json_line_shared(writer, &message);
}

fn emit_job_progress(writer: &SharedWriter, job: ActiveJob, payload: FfmpegProgressPayload) {
    emit_rpc_event(
        writer,
        "media.job.progress",
        json!({
            "jobId": job.job_id,
            "kind": job.kind,
            "progress": payload.progress,
            "step": payload.step,
        }),
    );
}

fn emit_job_error(writer: &SharedWriter, job: ActiveJob, error: &RpcErrorPayload) {
    emit_rpc_event(
        writer,
        "media.job.error",
        json!({
            "jobId": job.job_id,
            "kind": job.kind,
            "summary": error.summary,
            "detail": error.detail,
        }),
    );
}

fn emit_job_complete(writer: &SharedWriter, job: ActiveJob) {
    emit_rpc_event(
        writer,
        "media.job.complete",
        json!({
            "jobId": job.job_id,
            "kind": job.kind,
        }),
    );
}

fn parse_error_payload(err: &tiny_vid_core::error::AppError) -> RpcErrorPayload {
    match err {
        tiny_vid_core::error::AppError::Aborted => RpcErrorPayload {
            summary: "Aborted".to_string(),
            detail: "Aborted".to_string(),
        },
        tiny_vid_core::error::AppError::FfmpegFailed { code, stderr } if *code == -1 => {
            RpcErrorPayload {
                summary: stderr.clone(),
                detail: stderr.clone(),
            }
        }
        tiny_vid_core::error::AppError::FfmpegFailed { code, stderr } => {
            let parsed = tiny_vid_core::ffmpeg::parse_ffmpeg_error(stderr, Some(*code));
            RpcErrorPayload {
                summary: parsed.summary,
                detail: parsed.detail,
            }
        }
        _ => {
            let text = err.to_string();
            RpcErrorPayload {
                summary: text.clone(),
                detail: text,
            }
        }
    }
}

fn params_from_value<T: serde::de::DeserializeOwned>(
    params: Value,
) -> Result<T, tiny_vid_core::error::AppError> {
    serde_json::from_value(params)
        .map_err(|e| tiny_vid_core::error::AppError::from(format!("Invalid params payload: {}", e)))
}

fn is_async_request(method: &str, params: &Value) -> bool {
    if method != "media.process" {
        return false;
    }
    matches!(
        params.get("kind").and_then(Value::as_str),
        Some("preview") | Some("transcode")
    )
}

fn dispatch_sync(
    method: &str,
    params: Value,
    jobs: &JobState,
) -> Result<Value, tiny_vid_core::error::AppError> {
    match method {
        "app.capabilities" => {
            let result = sidecar_api::app_capabilities()?;
            serde_json::to_value(result).map_err(|e| {
                tiny_vid_core::error::AppError::from(format!(
                    "Failed to serialize app capabilities: {}",
                    e
                ))
            })
        }
        "media.inspect" => {
            let parsed: MediaInspectParams = params_from_value(params)?;
            match parsed {
                MediaInspectParams::Metadata { input_path } => {
                    let result = sidecar_api::get_video_metadata(input_path)?;
                    serde_json::to_value(result).map_err(|e| {
                        tiny_vid_core::error::AppError::from(format!(
                            "Failed to serialize metadata: {}",
                            e
                        ))
                    })
                }
                MediaInspectParams::CommandPreview {
                    options,
                    input_path,
                } => Ok(Value::String(sidecar_api::preview_ffmpeg_command(
                    options, input_path,
                ))),
            }
        }
        "media.process" => {
            let parsed: MediaProcessParams = params_from_value(params)?;
            match parsed {
                MediaProcessParams::Commit {
                    commit_token,
                    output_path,
                } => {
                    let saved_path =
                        sidecar_api::commit_transcode_output(commit_token, output_path)?;
                    Ok(json!({ "savedPath": saved_path }))
                }
                MediaProcessParams::Discard { commit_token } => {
                    sidecar_api::discard_transcode_output(commit_token)?;
                    Ok(json!({ "discarded": true }))
                }
                MediaProcessParams::Preview { .. } | MediaProcessParams::Transcode { .. } => {
                    Err(tiny_vid_core::error::AppError::from(
                        "media.process kind requires async execution",
                    ))
                }
            }
        }
        "media.cancel" => {
            let parsed: MediaCancelParams = params_from_value(params)?;
            let current_job = jobs.current_job();
            match (current_job, parsed.job_id) {
                (None, Some(job_id)) => Err(tiny_vid_core::error::AppError::from(format!(
                    "Unknown jobId: {}",
                    job_id
                ))),
                (None, None) => Ok(json!({ "cancelled": false, "jobId": Value::Null })),
                (Some(active), Some(job_id)) if active.job_id != job_id => Err(
                    tiny_vid_core::error::AppError::from(format!("Unknown jobId: {}", job_id)),
                ),
                (Some(active), _) => {
                    sidecar_api::ffmpeg_terminate();
                    Ok(json!({ "cancelled": true, "jobId": active.job_id }))
                }
            }
        }
        _ => Err(tiny_vid_core::error::AppError::from(format!(
            "Unknown method: {}",
            method
        ))),
    }
}

fn handle_sync_request(request: RpcRequest, writer: &SharedWriter, jobs: &JobState) {
    let response = match dispatch_sync(&request.method, request.params, jobs) {
        Ok(result) => serde_json::to_value(RpcSuccess {
            id: request.id,
            result,
        })
        .map_err(|e| io::Error::other(format!("serialize success: {}", e))),
        Err(err) => {
            let payload = parse_error_payload(&err);
            serde_json::to_value(RpcFailure {
                id: request.id,
                error: payload,
            })
            .map_err(|e| io::Error::other(format!("serialize failure: {}", e)))
        }
    };

    match response {
        Ok(value) => {
            let _ = write_json_line_shared(writer, &value);
        }
        Err(err) => {
            let failure = RpcFailure {
                id: request.id,
                error: RpcErrorPayload {
                    summary: "Serialization error".to_string(),
                    detail: err.to_string(),
                },
            };
            let _ = write_json_line_shared(writer, &failure);
        }
    }
}

fn handle_async_media_process(request: RpcRequest, writer: &SharedWriter, jobs: &JobState) {
    let parsed: MediaProcessParams = match params_from_value(request.params) {
        Ok(parsed) => parsed,
        Err(err) => {
            let payload = parse_error_payload(&err);
            let failure = RpcFailure {
                id: request.id,
                error: payload,
            };
            let _ = write_json_line_shared(writer, &failure);
            return;
        }
    };

    let kind = match parsed {
        MediaProcessParams::Preview { .. } => JobKind::Preview,
        MediaProcessParams::Transcode { .. } => JobKind::Transcode,
        _ => {
            let failure = RpcFailure {
                id: request.id,
                error: RpcErrorPayload {
                    summary: "Invalid media.process kind".to_string(),
                    detail: "Only preview and transcode are async media.process kinds".to_string(),
                },
            };
            let _ = write_json_line_shared(writer, &failure);
            return;
        }
    };

    let active_job = match jobs.begin_job(kind) {
        Ok(job) => job,
        Err(err) => {
            let payload = parse_error_payload(&err);
            let failure = RpcFailure {
                id: request.id,
                error: payload,
            };
            let _ = write_json_line_shared(writer, &failure);
            return;
        }
    };
    let _job_guard = ActiveJobGuard::new(jobs.clone(), active_job.job_id);

    let writer_for_events = Arc::clone(writer);
    let progress_emitter: sidecar_api::SidecarProgressEmitter =
        Arc::new(move |payload| emit_job_progress(&writer_for_events, active_job, payload));

    let result = match parsed {
        MediaProcessParams::Preview {
            input_path,
            options,
            preview_start_seconds,
            include_estimate,
        } => {
            let result = sidecar_api::ffmpeg_preview_with_events(
                input_path,
                options,
                preview_start_seconds,
                include_estimate,
                Some(Arc::clone(&progress_emitter)),
            );
            match result {
                Ok(value) => serde_json::to_value(value).map_err(|e| {
                    tiny_vid_core::error::AppError::from(format!(
                        "Failed to serialize preview result: {}",
                        e
                    ))
                }),
                Err(err) => Err(err),
            }
        }
        MediaProcessParams::Transcode {
            input_path,
            options,
        } => {
            let output_path_result = sidecar_api::ffmpeg_transcode_to_temp_with_events(
                input_path,
                options,
                Some(Arc::clone(&progress_emitter)),
            );
            match output_path_result {
                Ok(output_path) => {
                    match sidecar_api::register_transcode_commit(PathBuf::from(&output_path)) {
                        Ok(commit_token) => Ok(json!({
                            "jobId": active_job.job_id,
                            "commitToken": commit_token,
                        })),
                        Err(err) => Err(err),
                    }
                }
                Err(err) => Err(err),
            }
        }
        MediaProcessParams::Commit { .. } | MediaProcessParams::Discard { .. } => {
            Err(tiny_vid_core::error::AppError::from(
                "Commit/discard are synchronous media.process kinds",
            ))
        }
    };

    let response = match result {
        Ok(result) => {
            emit_job_complete(writer, active_job);
            serde_json::to_value(RpcSuccess {
                id: request.id,
                result,
            })
            .map_err(|e| io::Error::other(format!("serialize success: {}", e)))
        }
        Err(err) => {
            let payload = parse_error_payload(&err);
            emit_job_error(writer, active_job, &payload);
            serde_json::to_value(RpcFailure {
                id: request.id,
                error: payload,
            })
            .map_err(|e| io::Error::other(format!("serialize failure: {}", e)))
        }
    };

    match response {
        Ok(value) => {
            let _ = write_json_line_shared(writer, &value);
        }
        Err(err) => {
            let failure = RpcFailure {
                id: request.id,
                error: RpcErrorPayload {
                    summary: "Serialization error".to_string(),
                    detail: err.to_string(),
                },
            };
            let _ = write_json_line_shared(writer, &failure);
        }
    }
}

fn main() -> io::Result<()> {
    sidecar_api::cleanup_startup_temp(Duration::from_secs(STARTUP_CLEANUP_MAX_AGE_HOURS * 3600));

    let stdin = io::stdin();
    let stdout: SharedWriter = Arc::new(Mutex::new(io::stdout()));
    let jobs = JobState::new();
    let mut async_workers: Vec<thread::JoinHandle<()>> = Vec::new();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                let failure = RpcFailure {
                    id: 0,
                    error: RpcErrorPayload {
                        summary: "Invalid input stream".to_string(),
                        detail: err.to_string(),
                    },
                };
                let _ = write_json_line_shared(&stdout, &failure);
                continue;
            }
        };

        if line.trim().is_empty() {
            continue;
        }

        let request: RpcRequest = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(err) => {
                let failure = RpcFailure {
                    id: 0,
                    error: RpcErrorPayload {
                        summary: "Invalid request".to_string(),
                        detail: err.to_string(),
                    },
                };
                let _ = write_json_line_shared(&stdout, &failure);
                continue;
            }
        };

        if is_async_request(&request.method, &request.params) {
            let writer = Arc::clone(&stdout);
            let job_state = jobs.clone();
            let worker = thread::spawn(move || {
                handle_async_media_process(request, &writer, &job_state);
            });
            async_workers.push(worker);
        } else {
            handle_sync_request(request, &stdout, &jobs);
        }
    }

    for worker in async_workers {
        let _ = worker.join();
    }

    sidecar_api::cleanup_on_exit();
    Ok(())
}
