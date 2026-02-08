//! Map FFmpeg exit codes to user-friendly messages.
//!
//! Exit codes are from ffmpeg.c: 1 (general), 69 (rate exceeded),
//! 123 (hard exit), 255 (signal). -1 is used for spawn failure.
//! Stderr is kept as detail for debugging.

use serde::Serialize;

/// Payload for ffmpeg-error event. Frontend shows summary; detail is expandable.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegErrorPayload {
    pub summary: String,
    pub detail: String,
}

/// Maps FFmpeg exit code to a short user-facing summary. Stderr is passed through as detail.
pub fn parse_ffmpeg_error(stderr: &str, exit_code: Option<i32>) -> FfmpegErrorPayload {
    let summary = match exit_code {
        Some(code) => match known_exit_code_summary(code) {
            Some(msg) => msg,
            None => summary_for_unknown_code(code, stderr),
        },
        None => fallback_summary(stderr),
    };
    let detail = stderr.trim().to_string();
    FfmpegErrorPayload { summary, detail }
}

/// Source-verified exit codes from ffmpeg.c.
fn known_exit_code_summary(code: i32) -> Option<String> {
    match code {
        -1 => Some("FFmpeg not found or failed to start.".into()),
        1 => Some("FFmpeg failed.".into()),
        69 => Some("Encoding rate limit exceeded.".into()),
        123 | 255 => Some("Encoding was stopped.".into()),
        _ => None,
    }
}

/// Extract first non-empty line from stderr, truncate to max_len bytes (adding "…" if truncated).
const ELLIPSIS: &str = "…";

fn first_line_truncated(stderr: &str, max_len: usize) -> String {
    let first = stderr
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim())
        .unwrap_or(stderr);
    let max_chars = max_len.saturating_sub(ELLIPSIS.len());
    if first.len() <= max_len {
        first.to_string()
    } else {
        format!("{}{}", &first[..max_chars], ELLIPSIS)
    }
}

/// For unknown codes, use a short summary. Full stderr is in detail.
fn summary_for_unknown_code(code: i32, _stderr: &str) -> String {
    format!("FFmpeg failed (exit code {}).", code)
}

fn fallback_summary(stderr: &str) -> String {
    first_line_truncated(stderr, 120)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_1() {
        let p = parse_ffmpeg_error("", Some(1));
        assert_eq!(p.summary, "FFmpeg failed.");
    }

    #[test]
    fn exit_code_69() {
        let p = parse_ffmpeg_error("", Some(69));
        assert_eq!(p.summary, "Encoding rate limit exceeded.");
    }

    #[test]
    fn exit_code_255() {
        let p = parse_ffmpeg_error("", Some(255));
        assert_eq!(p.summary, "Encoding was stopped.");
    }

    #[test]
    fn exit_code_minus_one() {
        let p = parse_ffmpeg_error("Failed to spawn FFmpeg", Some(-1));
        assert!(p.summary.contains("not found") || p.summary.contains("start"));
    }

    #[test]
    fn unknown_code_short_summary() {
        let p = parse_ffmpeg_error("Invalid data found when processing input", Some(42));
        assert_eq!(p.summary, "FFmpeg failed (exit code 42).");
        assert_eq!(p.detail, "Invalid data found when processing input");
    }

    #[test]
    fn unknown_code_no_stderr() {
        let p = parse_ffmpeg_error("", Some(99));
        assert_eq!(p.summary, "FFmpeg failed (exit code 99).");
    }

    #[test]
    fn no_code_uses_stderr() {
        let p = parse_ffmpeg_error("Some random error\nSecond line", None);
        assert_eq!(p.summary, "Some random error");
    }

    #[test]
    fn long_stderr_truncated() {
        let long = "a".repeat(150);
        let p = parse_ffmpeg_error(&long, None);
        assert!(p.summary.len() <= 121);
        assert!(p.summary.ends_with('…'));
    }
}
