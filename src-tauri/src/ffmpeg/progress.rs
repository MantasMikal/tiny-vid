use regex::Regex;
use std::sync::LazyLock;

static DURATION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Duration: (\d+):(\d+):([\d.]+)").expect("invalid duration regex"));
static TIME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"out_time_ms=(\d+)").expect("invalid time regex"));

/// Parse FFmpeg progress output. Returns (progress 0.0-1.0 or None, duration in seconds or None).
pub fn parse_ffmpeg_progress(
    output: &str,
    current_duration: Option<f64>,
) -> (Option<f64>, Option<f64>) {
    if let Some(caps) = DURATION_RE.captures(output) {
        let hours: f64 = caps[1].parse().unwrap_or(0.0);
        let minutes: f64 = caps[2].parse().unwrap_or(0.0);
        let seconds: f64 = caps[3].parse().unwrap_or(0.0);
        let duration = hours * 3600.0 + minutes * 60.0 + seconds;
        return (None, Some(duration));
    }

    if let Some(caps) = TIME_RE.captures(output) {
        if let Some(dur) = current_duration {
            if dur > 0.0 {
                let current_time_ms: i64 = caps[1].parse().unwrap_or(0);
                let current_time = current_time_ms as f64 / 1_000_000.0;
                let progress = (current_time / dur).min(1.0);
                return (Some(progress), Some(dur));
            }
        }
    }

    (None, current_duration)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_parsed() {
        let (prog, dur) = parse_ffmpeg_progress("Duration: 0:1:30.5", None);
        assert_eq!(prog, None);
        assert_eq!(dur, Some(90.5));
    }

    #[test]
    fn duration_hours_minutes_seconds() {
        let (_, dur) = parse_ffmpeg_progress("Duration: 1:2:3.0", None);
        assert_eq!(dur, Some(3723.0));
    }

    #[test]
    fn out_time_ms_progress() {
        let (prog, dur) =
            parse_ffmpeg_progress("out_time_ms=5000000", Some(10.0));
        assert_eq!(prog, Some(0.5));
        assert_eq!(dur, Some(10.0));
    }

    #[test]
    fn out_time_ms_complete() {
        let (prog, dur) =
            parse_ffmpeg_progress("out_time_ms=10000000", Some(10.0));
        assert_eq!(prog, Some(1.0));
        assert_eq!(dur, Some(10.0));
    }

    #[test]
    fn invalid_line_returns_current_duration() {
        let (prog, dur) = parse_ffmpeg_progress("random garbage", Some(5.0));
        assert_eq!(prog, None);
        assert_eq!(dur, Some(5.0));
    }
}
