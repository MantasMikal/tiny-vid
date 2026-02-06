//! Build FFmpeg CLI args from TranscodeOptions. Maps quality/preset per codec (x264, x265, VP9, AV1, VideoToolbox).

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::error::AppError;
use super::TranscodeOptions;

/// Codec variant for FFmpeg argument construction. Each variant handles its own quality, preset, and tags.
#[derive(Clone, Copy)]
enum CodecKind {
    X264,
    X265,
    VP9,
    SvtAv1,
    VideoToolboxH264,
    VideoToolboxHevc,
}

impl CodecKind {
    fn from_codec_str(codec: &str) -> Self {
        let lower = codec.to_lowercase();
        if lower.contains("hevc_videotoolbox") {
            CodecKind::VideoToolboxHevc
        } else if lower.contains("h264_videotoolbox") {
            CodecKind::VideoToolboxH264
        } else if lower.contains("vp9") || lower.contains("vpx") {
            CodecKind::VP9
        } else if lower.contains("svtav1") {
            CodecKind::SvtAv1
        } else if (lower.contains("x265") || lower.contains("hevc")) && !lower.contains("videotoolbox")
        {
            CodecKind::X265
        } else {
            CodecKind::X264
        }
    }

    fn ffmpeg_name(&self) -> &'static str {
        match self {
            CodecKind::X264 => "libx264",
            CodecKind::X265 => "libx265",
            CodecKind::VP9 => "libvpx-vp9",
            CodecKind::SvtAv1 => "libsvtav1",
            CodecKind::VideoToolboxH264 => "h264_videotoolbox",
            CodecKind::VideoToolboxHevc => "hevc_videotoolbox",
        }
    }

    fn supports_tune(&self) -> bool {
        matches!(self, CodecKind::X264)
    }

    /// Build codec-specific args: preset/speed, quality/crf, tags, etc.
    fn build_codec_args(
        &self,
        quality: u32,
        preset: &str,
        tune: Option<&str>,
        max_bitrate: Option<u32>,
    ) -> Vec<String> {
        let mut args = Vec::new();

        match self {
            CodecKind::VP9 => {
                let (deadline, cpu_used) = VP9_CPU_USED_MAP
                    .get(preset)
                    .copied()
                    .unwrap_or(("good", "2"));
                args.extend(["-deadline".to_string(), deadline.to_string()]);
                args.extend(["-cpu-used".to_string(), cpu_used.to_string()]);
                args.extend(["-row-mt".to_string(), "1".to_string()]);
                args.extend(["-b:v".to_string(), "0".to_string()]);
            }
            CodecKind::SvtAv1 => {
                let preset_val = SVTAV1_PRESET_MAP.get(preset).unwrap_or(&"8");
                args.extend(["-preset".to_string(), preset_val.to_string()]);
                args.extend(["-pix_fmt".to_string(), "yuv420p".to_string()]);
                args.extend(["-tag:v".to_string(), "av01".to_string()]);
            }
            CodecKind::VideoToolboxH264 | CodecKind::VideoToolboxHevc => {
                args.extend(["-q:v".to_string(), quality.min(100).to_string()]);
                if let Some(max_br) = max_bitrate {
                    args.extend([
                        "-maxrate".to_string(),
                        format!("{}k", max_br),
                        "-bufsize".to_string(),
                        format!("{}k", max_br * 2),
                    ]);
                }
                if matches!(self, CodecKind::VideoToolboxHevc) {
                    args.extend(["-tag:v".to_string(), "hvc1".to_string()]);
                }
            }
            CodecKind::X264 | CodecKind::X265 => {
                args.extend(["-preset".to_string(), preset.to_string()]);
                if matches!(self, CodecKind::X265) {
                    args.extend(["-tag:v".to_string(), "hvc1".to_string()]);
                }
            }
        }

        if self.supports_tune()
            && let Some(tune_val) = tune
                && !tune_val.is_empty() && tune_val != "none" {
                    args.extend(["-tune".to_string(), tune_val.to_string()]);
                }

        match self {
            CodecKind::X264 | CodecKind::X265 | CodecKind::VP9 | CodecKind::SvtAv1 => {
                let crf = match self {
                    CodecKind::X265 => map_linear_crf(quality, 28, 51),
                    CodecKind::SvtAv1 => map_linear_crf(quality, 24, 63),
                    CodecKind::VP9 => map_linear_crf(quality, 20, 63),
                    _ => map_linear_crf(quality, 23, 51),
                };
                if let Some(max_br) = max_bitrate {
                    args.extend([
                        "-crf".to_string(),
                        crf.to_string(),
                        "-maxrate".to_string(),
                        format!("{}k", max_br),
                        "-bufsize".to_string(),
                        format!("{}k", max_br * 2),
                    ]);
                } else {
                    args.extend(["-crf".to_string(), crf.to_string()]);
                }
            }
            _ => {}
        }

        args
    }
}

/// libsvtav1 preset: 0-13 (higher = faster). Maps x264-style preset names.
static SVTAV1_PRESET_MAP: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    [
        ("ultrafast", "12"),
        ("superfast", "11"),
        ("veryfast", "10"),
        ("faster", "9"),
        ("fast", "8"),
        ("medium", "6"),
        ("slow", "4"),
    ]
    .into_iter()
    .collect()
});

/// libvpx-vp9 -cpu-used: 0-5 (0=slowest/best, 5=fastest). Maps x264-style preset names.
/// -deadline good with cpu-used. For "slow" we use deadline best.
static VP9_CPU_USED_MAP: LazyLock<HashMap<&'static str, (&'static str, &'static str)>> =
    LazyLock::new(|| {
        [
            ("ultrafast", ("good", "4")),
            ("superfast", ("good", "4")),
            ("veryfast", ("good", "3")),
            ("faster", ("good", "3")),
            ("fast", ("good", "2")),
            ("medium", ("good", "1")),
            ("slow", ("best", "0")),
        ]
        .into_iter()
        .collect()
    });

fn map_linear_crf(quality: u32, high_crf: i32, low_crf: i32) -> i32 {
    let q = quality.min(100) as f64 / 100.0;
    (low_crf as f64 - q * (low_crf - high_crf) as f64).round() as i32
}

/// Per-format audio and container settings (MP4, WebM, MKV).
#[derive(Clone, Copy)]
struct OutputFormatConfig {
    audio_codec: &'static str,
    requires_stereo_downmix: bool,
    use_movflags_faststart: bool,
    supports_multiple_audio: bool,
}

impl OutputFormatConfig {
    /// Returns true if source audio can be passed through (copy) instead of re-encoding.
    fn can_passthrough_audio(
        &self,
        source_codec: Option<&str>,
        source_channels: Option<u32>,
        downmix: bool,
    ) -> bool {
        let Some(codec) = source_codec else {
            return false;
        };
        let codec_lower = codec.to_lowercase();
        let codec_matches = match self.audio_codec {
            "aac" => codec_lower == "aac" || codec_lower == "aac_latm",
            "libopus" => codec_lower == "opus",
            _ => return false,
        };
        if !codec_matches {
            return false;
        }
        if self.requires_stereo_downmix || downmix {
            source_channels == Some(2)
        } else {
            true
        }
    }
}

fn get_output_config(format: &str, video_codec: &str) -> OutputFormatConfig {
    let is_vp9 = video_codec.to_lowercase().contains("vp9");
    match (format.to_lowercase().as_str(), is_vp9) {
        ("mp4", _) => OutputFormatConfig {
            audio_codec: "aac",
            requires_stereo_downmix: false,
            use_movflags_faststart: true,
            supports_multiple_audio: true,
        },
        ("webm", _) => OutputFormatConfig {
            audio_codec: "libopus",
            requires_stereo_downmix: true,
            use_movflags_faststart: false,
            supports_multiple_audio: false,
        },
        ("mkv", true) => OutputFormatConfig {
            audio_codec: "libopus",
            requires_stereo_downmix: true,
            use_movflags_faststart: false,
            supports_multiple_audio: true,
        },
        ("mkv", false) => OutputFormatConfig {
            audio_codec: "aac",
            requires_stereo_downmix: false,
            use_movflags_faststart: false,
            supports_multiple_audio: true,
        },
        _ => OutputFormatConfig {
            audio_codec: "aac",
            requires_stereo_downmix: false,
            use_movflags_faststart: true,
            supports_multiple_audio: true,
        },
    }
}

/// Returns true if the codec is widely playable in browsers (H.264, HEVC, VP9, AV1).
pub fn is_browser_playable_codec(codec_name: &str) -> bool {
    let lower = codec_name.to_lowercase();
    matches!(
        lower.as_str(),
        "h264" | "avc" | "avc1" | "hevc" | "h265" | "vp9" | "av1"
    )
}

/// Base args shared by FFmpeg invocations: nostdin, threads, thread_queue_size.
fn ffmpeg_base_args() -> Vec<String> {
    vec![
        "-nostdin".to_string(),
        "-threads".to_string(),
        "0".to_string(),
        "-thread_queue_size".to_string(),
        "512".to_string(),
    ]
}

/// Build args for segment extraction (-ss -t -i -c copy).
pub fn build_extract_args(
    input_path: &str,
    start_secs: f64,
    duration_secs: f64,
    output_path: &str,
) -> Vec<String> {
    let mut args = ffmpeg_base_args();
    args.extend([
        "-ss".to_string(),
        start_secs.to_string(),
        "-t".to_string(),
        duration_secs.to_string(),
        "-progress".to_string(),
        "pipe:1".to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-c".to_string(),
        "copy".to_string(),
        "-avoid_negative_ts".to_string(),
        "make_zero".to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        output_path.to_string(),
    ]);
    args
}

/// Build FFmpeg transcode command.
pub fn build_ffmpeg_command(
    input_path: &str,
    output_path: &str,
    options: &TranscodeOptions,
    output_duration_secs: Option<f64>,
    format_override: Option<&str>,
    start_offset_secs: Option<f64>,
) -> Result<Vec<String>, AppError> {
    let output_format = format_override
        .map(str::to_lowercase)
        .unwrap_or_else(|| options.effective_output_format());

    let codec_str = options.effective_codec().to_string();
    let codec_kind = CodecKind::from_codec_str(&codec_str);
    let quality = options.effective_quality();
    let max_bitrate = options.max_bitrate;
    let scale = options.effective_scale();
    let fps = options.effective_fps();
    let remove_audio = options.effective_remove_audio();
    let preset = options.effective_preset();
    let tune = options.effective_tune();

    log::debug!(
        target: "tiny_vid::ffmpeg::builder",
        "Building FFmpeg command: codec={}, preset={}, input={} -> output={}",
        codec_kind.ffmpeg_name(),
        preset,
        input_path,
        output_path
    );

    let config = get_output_config(&output_format, &codec_str);
    // Preview uses format_override (e.g. "mp4"); always single audio. Export honors preserve.
    let is_preview = format_override.is_some();
    let preserve_multi = !is_preview
        && config.supports_multiple_audio
        && options.effective_preserve_additional_audio_streams()
        && options.effective_audio_stream_count() > 1;
    let preserve_subtitles =
        options.effective_preserve_subtitles() && options.effective_subtitle_stream_count() > 0;
    let use_explicit_mapping = preserve_multi || preserve_subtitles;

    let audio_bitrate_k = format!("{}k", options.effective_audio_bitrate());
    let downmix = options.effective_downmix_to_stereo();
    let passthrough = !preserve_multi
        && config.can_passthrough_audio(
            options.audio_codec_name.as_deref(),
            options.audio_channels,
            downmix,
        );

    let mut args = ffmpeg_base_args();
    args.extend(["-progress".to_string(), "pipe:1".to_string()]);
    if let Some(ss) = start_offset_secs.filter(|&s| s > 0.0) {
        args.extend(["-ss".to_string(), ss.to_string()]);
    }
    args.extend(["-i".to_string(), input_path.to_string()]);

    if use_explicit_mapping {
        args.push("-map".to_string());
        args.push("0:v".to_string());
        let n = options.effective_audio_stream_count();
        if preserve_multi {
            for i in 0..n {
                args.push("-map".to_string());
                args.push(format!("0:a:{}", i));
            }
        } else {
            args.push("-map".to_string());
            args.push("0:a:0".to_string());
        }
        if preserve_subtitles {
            args.push("-map".to_string());
            args.push("0:s".to_string());
        }
    }

    args.extend([
        "-c:v".to_string(),
        codec_kind.ffmpeg_name().to_string(),
    ]);

    if remove_audio {
        args.push("-an".to_string());
    } else if preserve_multi {
        let n = options.effective_audio_stream_count();
        for i in 0..n {
            if passthrough {
                args.extend([format!("-c:a:{}", i), "copy".to_string()]);
            } else {
                args.extend([
                    format!("-c:a:{}", i),
                    config.audio_codec.to_string(),
                    format!("-b:a:{}", i),
                    audio_bitrate_k.clone(),
                ]);
                if config.requires_stereo_downmix || downmix {
                    args.extend([format!("-ac:a:{}", i), "2".to_string()]);
                }
            }
        }
    } else if config.requires_stereo_downmix {
        if passthrough {
            args.extend(["-c:a".to_string(), "copy".to_string()]);
        } else {
            args.extend([
                "-c:a".to_string(),
                config.audio_codec.to_string(),
                "-b:a".to_string(),
                audio_bitrate_k.clone(),
                "-ac".to_string(),
                "2".to_string(),
            ]);
        }
    } else if passthrough {
        args.extend(["-c:a".to_string(), "copy".to_string()]);
    } else {
        let mut audio_args = vec![
            "-c:a".to_string(),
            config.audio_codec.to_string(),
            "-b:a".to_string(),
            audio_bitrate_k,
        ];
        if downmix {
            audio_args.extend(["-ac".to_string(), "2".to_string()]);
        }
        args.extend(audio_args);
    }

    if scale < 1.0 {
        let scale_filter = format!("scale=round(iw*{}/2)*2:-2", scale);
        args.extend(["-vf".to_string(), scale_filter]);
    }

    args.extend(codec_kind.build_codec_args(quality, preset, tune, max_bitrate));

    args.extend(["-r".to_string(), fps.to_string()]);
    if config.use_movflags_faststart {
        args.extend(["-movflags".to_string(), "+faststart".to_string()]);
    }

    if let Some(dur) = output_duration_secs.filter(|&d| d > 0.0) {
        args.extend(["-t".to_string(), dur.to_string()]);
    }
    if options.effective_preserve_metadata() {
        args.extend(["-map_metadata".to_string(), "0".to_string()]);
    }
    args.push(output_path.to_string());
    Ok(args)
}

/// Formats args for readable display: option and value on the same line when the next arg is a value.
pub fn format_args_for_display_multiline(args: &[String]) -> String {
    if args.is_empty() {
        return String::new();
    }
    let mut lines = Vec::new();
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        let line = if arg.starts_with('-')
            && iter.peek().is_some_and(|next| !next.starts_with('-'))
        {
            let value = iter.next().unwrap_or_else(|| unreachable!("peek confirmed next arg exists"));
            format!("  {} {}", arg, value)
        } else {
            format!("  {}", arg)
        };
        lines.push(line);
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> TranscodeOptions {
        TranscodeOptions::default()
    }

    #[test]
    fn build_extract_args_includes_faststart_and_avoid_negative_ts() {
        let args = build_extract_args("/in.mkv", 0.0, 3.0, "/out.mp4");
        assert!(args.contains(&"-movflags".to_string()));
        assert!(args.contains(&"+faststart".to_string()));
        assert!(args.contains(&"-avoid_negative_ts".to_string()));
        assert!(args.contains(&"make_zero".to_string()));
        assert!(args.contains(&"-c".to_string()));
        assert!(args.contains(&"copy".to_string()));
    }

    #[test]
    fn default_options_produces_expected_args() {
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &opts(), None, None, None).unwrap();
        assert!(args.contains(&"-i".to_string()));
        assert!(args.contains(&"/in.mp4".to_string()));
        assert!(args.iter().any(|a| a == "-c:v"));
        assert!(args.contains(&"libx264".to_string()));
        assert!(args.contains(&"-preset".to_string()));
        assert!(args.contains(&"fast".to_string()));
        assert!(args.contains(&"-r".to_string()));
        assert!(args.contains(&"30".to_string()));
        assert!(args.contains(&"-movflags".to_string()));
        assert!(args.contains(&"+faststart".to_string()));
        assert!(args.last() == Some(&"/out.mp4".to_string()));
    }

    #[test]
    fn scale_below_one_adds_scale_filter() {
        let mut o = opts();
        o.scale = Some(0.5);
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        assert!(args.contains(&"-vf".to_string()));
        let vf_idx = args.iter().position(|a| a == "-vf").unwrap();
        assert_eq!(
            args.get(vf_idx + 1).unwrap(),
            "scale=round(iw*0.5/2)*2:-2"
        );
    }

    #[test]
    fn remove_audio_adds_an() {
        let mut o = opts();
        o.remove_audio = Some(true);
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        assert!(args.contains(&"-an".to_string()));
        assert!(!args.iter().any(|a| a == "-c:a"));
    }

    #[test]
    fn h264_quality_maps_to_crf() {
        let mut o = opts();
        o.quality = Some(0);
        o.codec = Some("libx264".to_string());
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        let crf_idx = args.iter().position(|a| a == "-crf").unwrap();
        assert_eq!(args.get(crf_idx + 1).unwrap(), "51");
    }

    #[test]
    fn h265_quality_uses_different_crf_range() {
        let mut o = opts();
        o.quality = Some(100);
        o.codec = Some("libx265".to_string());
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        let crf_idx = args.iter().position(|a| a == "-crf").unwrap();
        assert_eq!(args.get(crf_idx + 1).unwrap(), "28");
    }

    #[test]
    fn h265_adds_hvc1_tag_for_quicktime() {
        let mut o = opts();
        o.codec = Some("libx265".to_string());
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        assert!(args.contains(&"-tag:v".to_string()));
        let tag_idx = args.iter().position(|a| a == "-tag:v").unwrap();
        assert_eq!(args.get(tag_idx + 1).unwrap(), "hvc1");
    }

    #[test]
    fn svtav1_preset_map() {
        let mut o = opts();
        o.codec = Some("libsvtav1".to_string());
        o.preset = Some("ultrafast".to_string());
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        assert!(args.contains(&"-preset".to_string()));
        let preset_idx = args.iter().position(|a| a == "-preset").unwrap();
        assert_eq!(args.get(preset_idx + 1).unwrap(), "12");
    }

    #[test]
    fn max_bitrate_adds_maxrate_and_bufsize() {
        let mut o = opts();
        o.max_bitrate = Some(2000);
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        assert!(args.contains(&"-maxrate".to_string()));
        assert!(args.contains(&"-bufsize".to_string()));
        let maxrate_idx = args.iter().position(|a| a == "-maxrate").unwrap();
        assert_eq!(args.get(maxrate_idx + 1).unwrap(), "2000k");
        let bufsize_idx = args.iter().position(|a| a == "-bufsize").unwrap();
        assert_eq!(args.get(bufsize_idx + 1).unwrap(), "4000k");
    }

    #[test]
    fn svtav1_quality_uses_linear_range() {
        let mut o = opts();
        o.codec = Some("libsvtav1".to_string());
        o.quality = Some(0);
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        let crf_idx = args.iter().position(|a| a == "-crf").unwrap();
        assert_eq!(args.get(crf_idx + 1).unwrap(), "63");
        o.quality = Some(100);
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        let crf_idx = args.iter().position(|a| a == "-crf").unwrap();
        assert_eq!(args.get(crf_idx + 1).unwrap(), "24");
    }

    #[test]
    fn quality_75_maps_to_different_crf_per_codec() {
        let mut o = opts();
        o.quality = Some(75);
        o.codec = Some("libx264".to_string());
        let args_h264 = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        o.codec = Some("libsvtav1".to_string());
        let args_av1 = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        let crf_idx_h264 = args_h264.iter().position(|a| a == "-crf").unwrap();
        let crf_idx_av1 = args_av1.iter().position(|a| a == "-crf").unwrap();
        assert_eq!(args_h264.get(crf_idx_h264 + 1).unwrap(), "30");
        assert_eq!(args_av1.get(crf_idx_av1 + 1).unwrap(), "34");
    }

    #[test]
    fn tune_added_for_x264() {
        let mut o = opts();
        o.tune = Some("film".to_string());
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        assert!(args.contains(&"-tune".to_string()));
        let tune_idx = args.iter().position(|a| a == "-tune").unwrap();
        assert_eq!(args.get(tune_idx + 1).unwrap(), "film");
    }

    #[test]
    fn tune_skipped_for_svtav1() {
        let mut o = opts();
        o.codec = Some("libsvtav1".to_string());
        o.tune = Some("film".to_string());
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        assert!(!args.contains(&"-tune".to_string()));
    }

    #[test]
    fn svtav1_adds_pix_fmt_and_av01_tag() {
        let mut o = opts();
        o.codec = Some("libsvtav1".to_string());
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        assert!(args.contains(&"-pix_fmt".to_string()));
        let pix_idx = args.iter().position(|a| a == "-pix_fmt").unwrap();
        assert_eq!(args.get(pix_idx + 1).unwrap(), "yuv420p");
        assert!(args.contains(&"-tag:v".to_string()));
        let tag_idx = args.iter().position(|a| a == "-tag:v").unwrap();
        assert_eq!(args.get(tag_idx + 1).unwrap(), "av01");
    }

    #[test]
    fn tune_none_omitted() {
        let o = opts();
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        assert!(!args.contains(&"-tune".to_string()));
    }

    #[test]
    fn svtav1_preset_map_all_entries() {
        let presets = [
            ("superfast", "11"),
            ("veryfast", "10"),
            ("faster", "9"),
            ("fast", "8"),
            ("medium", "6"),
            ("slow", "4"),
        ];
        for (preset_name, expected) in presets {
            let mut o = opts();
            o.codec = Some("libsvtav1".to_string());
            o.preset = Some(preset_name.to_string());
            let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
            let preset_idx = args.iter().position(|a| a == "-preset").unwrap();
            assert_eq!(
                args.get(preset_idx + 1).unwrap(),
                expected,
                "preset {} should map to {}",
                preset_name,
                expected
            );
        }
    }

    #[test]
    fn svtav1_unknown_preset_fallback() {
        let mut o = opts();
        o.codec = Some("libsvtav1".to_string());
        o.preset = Some("veryslow".to_string());
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        let preset_idx = args.iter().position(|a| a == "-preset").unwrap();
        assert_eq!(args.get(preset_idx + 1).unwrap(), "8");
    }

    #[test]
    fn custom_fps_passthrough() {
        let mut o = opts();
        o.fps = Some(24.0);
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        let r_idx = args.iter().position(|a| a == "-r").unwrap();
        assert_eq!(args.get(r_idx + 1).unwrap(), "24");
        o.fps = Some(60.0);
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        let r_idx = args.iter().position(|a| a == "-r").unwrap();
        assert_eq!(args.get(r_idx + 1).unwrap(), "60");
    }

    #[test]
    fn scale_one_no_vf() {
        let mut o = opts();
        o.scale = Some(1.0);
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        assert!(!args.contains(&"-vf".to_string()));
    }

    #[test]
    #[cfg(not(feature = "lgpl"))]
    fn webm_uses_libopus_no_movflags() {
        let mut o = opts();
        o.codec = Some("libsvtav1".to_string());
        o.output_format = Some("webm".to_string());
        o.remove_audio = Some(false);
        let args = build_ffmpeg_command("/in.mp4", "/out.webm", &o, None, None, None).unwrap();
        assert!(args.contains(&"libopus".to_string()));
        assert!(args.contains(&"-ac".to_string()), "WebM+Opus should downmix to stereo (-ac 2)");
        assert!(!args.contains(&"-movflags".to_string()));
        assert!(args.last() == Some(&"/out.webm".to_string()));
    }

    #[test]
    #[cfg(not(feature = "lgpl"))]
    fn webm_no_audio_uses_an() {
        let mut o = opts();
        o.codec = Some("libsvtav1".to_string());
        o.output_format = Some("webm".to_string());
        o.remove_audio = Some(true);
        let args = build_ffmpeg_command("/in.mp4", "/out.webm", &o, None, None, None).unwrap();
        assert!(args.contains(&"-an".to_string()));
        assert!(!args.contains(&"-ac".to_string()), "No audio: -an, not -ac");
        assert!(!args.contains(&"-movflags".to_string()));
    }

    #[test]
    #[cfg(not(feature = "lgpl"))]
    fn vp9_uses_deadline_cpu_used_bv0() {
        let mut o = opts();
        o.codec = Some("libvpx-vp9".to_string());
        o.output_format = Some("webm".to_string());
        o.preset = Some("fast".to_string());
        let args = build_ffmpeg_command("/in.mp4", "/out.webm", &o, None, None, None).unwrap();
        assert!(args.contains(&"libvpx-vp9".to_string()));
        assert!(args.contains(&"-deadline".to_string()));
        assert!(args.contains(&"-cpu-used".to_string()));
        assert!(args.contains(&"-b:v".to_string()));
        let bv_idx = args.iter().position(|a| a == "-b:v").unwrap();
        assert_eq!(args.get(bv_idx + 1).unwrap(), "0");
        assert!(!args.contains(&"-preset".to_string()));
        assert!(args.contains(&"libopus".to_string()));
    }

    #[test]
    #[cfg(not(feature = "lgpl"))]
    fn vp9_quality_maps_to_crf() {
        let mut o = opts();
        o.codec = Some("libvpx-vp9".to_string());
        o.output_format = Some("webm".to_string());
        o.quality = Some(0);
        let args = build_ffmpeg_command("/in.mp4", "/out.webm", &o, None, None, None).unwrap();
        let crf_idx = args.iter().position(|a| a == "-crf").unwrap();
        assert_eq!(args.get(crf_idx + 1).unwrap(), "63", "quality 0 -> worst CRF");
        o.quality = Some(100);
        let args2 = build_ffmpeg_command("/in.mp4", "/out.webm", &o, None, None, None).unwrap();
        let crf_idx2 = args2.iter().position(|a| a == "-crf").unwrap();
        assert_eq!(args2.get(crf_idx2 + 1).unwrap(), "20", "quality 100 -> best CRF");
    }

    #[test]
    fn h264_videotoolbox_uses_qv_not_crf() {
        let mut o = opts();
        o.codec = Some("h264_videotoolbox".to_string());
        o.quality = Some(75);
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        assert!(args.contains(&"-q:v".to_string()), "VideoToolbox should use -q:v");
        let qv_idx = args.iter().position(|a| a == "-q:v").unwrap();
        assert_eq!(args.get(qv_idx + 1).unwrap(), "75", "quality 75 -> -q:v 75");
        assert!(!args.contains(&"-crf".to_string()), "VideoToolbox should not use -crf");
    }

    #[test]
    fn h264_videotoolbox_no_preset_no_tune() {
        let mut o = opts();
        o.codec = Some("h264_videotoolbox".to_string());
        o.preset = Some("fast".to_string());
        o.tune = Some("film".to_string());
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        assert!(!args.contains(&"-preset".to_string()), "VideoToolbox should not use -preset");
        assert!(!args.contains(&"-tune".to_string()), "VideoToolbox should not use -tune");
    }

    #[test]
    fn videotoolbox_quality_100_is_qv_100() {
        let mut o = opts();
        o.codec = Some("h264_videotoolbox".to_string());
        o.quality = Some(100);
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        let qv_idx = args.iter().position(|a| a == "-q:v").unwrap();
        assert_eq!(args.get(qv_idx + 1).unwrap(), "100", "quality 100 -> -q:v 100 (best)");
    }

    #[test]
    fn videotoolbox_quality_0_is_qv_0() {
        let mut o = opts();
        o.codec = Some("h264_videotoolbox".to_string());
        o.quality = Some(0);
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        let qv_idx = args.iter().position(|a| a == "-q:v").unwrap();
        assert_eq!(args.get(qv_idx + 1).unwrap(), "0", "quality 0 -> -q:v 0 (worst)");
    }

    #[test]
    #[cfg(not(feature = "lgpl"))]
    fn mkv_uses_aac_no_movflags() {
        let mut o = opts();
        o.output_format = Some("mkv".to_string());
        o.codec = Some("libx264".to_string());
        o.remove_audio = Some(false);
        let args = build_ffmpeg_command("/in.mp4", "/out.mkv", &o, None, None, None).unwrap();
        assert!(args.contains(&"aac".to_string()));
        assert!(!args.contains(&"-movflags".to_string()));
        assert!(args.last() == Some(&"/out.mkv".to_string()));
    }

    #[test]
    #[cfg(not(feature = "lgpl"))]
    fn mkv_vp9_uses_opus() {
        let mut o = opts();
        o.output_format = Some("mkv".to_string());
        o.codec = Some("libvpx-vp9".to_string());
        o.remove_audio = Some(false);
        let args = build_ffmpeg_command("/in.mp4", "/out.mkv", &o, None, None, None).unwrap();
        assert!(args.contains(&"libopus".to_string()));
        assert!(!args.contains(&"-movflags".to_string()));
    }

    #[test]
    #[cfg(feature = "lgpl")]
    fn lgpl_accepts_mkv_output() {
        let mut o = opts();
        o.output_format = Some("mkv".to_string());
        o.codec = Some("h264_videotoolbox".to_string());
        let result = build_ffmpeg_command("/in.mp4", "/out.mkv", &o, None, None, None);
        assert!(result.is_ok(), "lgpl build should accept MKV output: {:?}", result.err());
    }

    #[test]
    fn preserve_additional_audio_streams_adds_map_and_per_track_codec() {
        let mut o = opts();
        o.preserve_additional_audio_streams = Some(true);
        o.audio_stream_count = Some(3);
        o.remove_audio = Some(false);
        o.output_format = Some("mp4".to_string());
        let args = build_ffmpeg_command("/in.mkv", "/out.mp4", &o, None, None, None).unwrap();
        assert!(args.contains(&"-map".to_string()));
        assert!(args.contains(&"0:v".to_string()));
        assert!(args.contains(&"0:a:0".to_string()));
        assert!(args.contains(&"0:a:1".to_string()));
        assert!(args.contains(&"0:a:2".to_string()));
        assert!(args.contains(&"-c:a:0".to_string()));
        assert!(args.contains(&"-c:a:1".to_string()));
        assert!(args.contains(&"-c:a:2".to_string()));
        assert!(args.contains(&"aac".to_string()));
    }

    #[test]
    fn preserve_additional_audio_streams_ignored_for_preview() {
        let mut o = opts();
        o.preserve_additional_audio_streams = Some(true);
        o.audio_stream_count = Some(3);
        o.remove_audio = Some(false);
        let args = build_ffmpeg_command(
            "/in.mkv",
            "/out.mp4",
            &o,
            Some(3.0),
            Some("mp4"),
            None,
        )
        .unwrap();
        assert!(!args.contains(&"0:a:1".to_string()), "Preview uses single audio");
    }

    #[test]
    #[cfg(not(feature = "lgpl"))]
    fn preserve_additional_audio_streams_ignored_for_webm() {
        let mut o = opts();
        o.preserve_additional_audio_streams = Some(true);
        o.audio_stream_count = Some(3);
        o.remove_audio = Some(false);
        o.output_format = Some("webm".to_string());
        o.codec = Some("libvpx-vp9".to_string());
        let args = build_ffmpeg_command("/in.mkv", "/out.webm", &o, None, None, None).unwrap();
        assert!(!args.contains(&"0:a:1".to_string()), "WebM supports single audio only");
    }

    #[test]
    #[cfg(not(feature = "lgpl"))]
    fn mkv_vp9_preserve_additional_audio_streams_downmixes_each_track() {
        let mut o = opts();
        o.preserve_additional_audio_streams = Some(true);
        o.audio_stream_count = Some(2);
        o.remove_audio = Some(false);
        o.output_format = Some("mkv".to_string());
        o.codec = Some("libvpx-vp9".to_string());
        let args = build_ffmpeg_command("/in.mkv", "/out.mkv", &o, None, None, None).unwrap();
        assert!(args.contains(&"-ac:a:0".to_string()));
        assert!(args.contains(&"-ac:a:1".to_string()));
        let ac0_idx = args.iter().position(|a| a == "-ac:a:0").unwrap();
        let ac1_idx = args.iter().position(|a| a == "-ac:a:1").unwrap();
        assert_eq!(args.get(ac0_idx + 1).unwrap(), "2");
        assert_eq!(args.get(ac1_idx + 1).unwrap(), "2");
    }

    #[test]
    fn hevc_videotoolbox_same_as_h264_vt() {
        let mut o = opts();
        o.codec = Some("hevc_videotoolbox".to_string());
        o.quality = Some(50);
        o.preset = Some("fast".to_string());
        o.tune = Some("film".to_string());
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        assert!(args.contains(&"-q:v".to_string()));
        assert!(!args.contains(&"-preset".to_string()));
        assert!(!args.contains(&"-tune".to_string()));
        assert!(args.contains(&"-tag:v".to_string()));
        let tag_idx = args.iter().position(|a| a == "-tag:v").unwrap();
        assert_eq!(args.get(tag_idx + 1).unwrap(), "hvc1");
    }

    #[test]
    fn audio_bitrate_used_in_args() {
        let mut o = opts();
        o.audio_bitrate = Some(192);
        o.remove_audio = Some(false);
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        let ba_idx = args.iter().position(|a| a == "-b:a").unwrap();
        assert_eq!(args.get(ba_idx + 1).unwrap(), "192k");
    }

    #[test]
    fn downmix_to_stereo_adds_ac() {
        let mut o = opts();
        o.downmix_to_stereo = Some(true);
        o.remove_audio = Some(false);
        o.output_format = Some("mp4".to_string());
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        assert!(args.contains(&"-ac".to_string()));
        let ac_idx = args.iter().position(|a| a == "-ac").unwrap();
        assert_eq!(args.get(ac_idx + 1).unwrap(), "2");
    }

    #[test]
    fn preserve_subtitles_adds_map_s() {
        let mut o = opts();
        o.preserve_subtitles = Some(true);
        o.subtitle_stream_count = Some(2);
        o.remove_audio = Some(false);
        let args = build_ffmpeg_command("/in.mkv", "/out.mp4", &o, None, None, None).unwrap();
        assert!(args.contains(&"-map".to_string()));
        assert!(args.contains(&"0:s".to_string()));
    }

    #[test]
    fn audio_passthrough_uses_copy() {
        let mut o = opts();
        o.audio_codec_name = Some("aac".to_string());
        o.audio_channels = Some(2);
        o.remove_audio = Some(false);
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        assert!(args.contains(&"-c:a".to_string()));
        let ca_idx = args.iter().position(|a| a == "-c:a").unwrap();
        assert_eq!(args.get(ca_idx + 1).unwrap(), "copy");
    }

    #[test]
    fn preserve_metadata_adds_map_metadata() {
        let mut o = opts();
        o.preserve_metadata = Some(true);
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o, None, None, None).unwrap();
        assert!(args.contains(&"-map_metadata".to_string()));
        let mm_idx = args.iter().position(|a| a == "-map_metadata").unwrap();
        assert_eq!(args.get(mm_idx + 1).unwrap(), "0");
    }
}
