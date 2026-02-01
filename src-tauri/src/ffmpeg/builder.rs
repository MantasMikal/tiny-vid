use std::collections::HashMap;
use std::sync::LazyLock;

use super::TranscodeOptions;

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

fn get_quality(quality: u32, codec_lower: &str) -> i32 {
    if codec_lower.contains("x265") || codec_lower.contains("hevc") {
        return map_linear_crf(quality, 28, 51);
    }
    if codec_lower.contains("svtav1") {
        return map_linear_crf(quality, 24, 63);
    }
    if codec_lower.contains("vp9") || codec_lower.contains("vpx") {
        return map_linear_crf(quality, 20, 63);
    }
    map_linear_crf(quality, 23, 51)
}

fn get_codec_preset(preset: &str, codec_lower: &str) -> String {
    if codec_lower.contains("svtav1") {
        return SVTAV1_PRESET_MAP.get(preset).unwrap_or(&"8").to_string();
    }
    preset.to_string()
}

/// For VP9: returns (deadline, cpu_used). For other codecs returns None.
fn get_vp9_speed(preset: &str, codec_lower: &str) -> Option<(&'static str, &'static str)> {
    if codec_lower.contains("vp9") || codec_lower.contains("vpx") {
        VP9_CPU_USED_MAP
            .get(preset)
            .copied()
            .or(Some(("good", "2")))
    } else {
        None
    }
}

pub fn build_ffmpeg_command(
    input_path: &str,
    output_path: &str,
    options: &TranscodeOptions,
) -> Vec<String> {
    let codec = options
        .codec
        .as_deref()
        .unwrap_or("libx264")
        .to_string();
    let codec_lower = codec.to_lowercase();
    let quality = options.quality.unwrap_or(75);
    let max_bitrate = options.max_bitrate;
    let scale = options.scale.unwrap_or(1.0);
    let fps = options.fps.unwrap_or(30);
    let remove_audio = options.remove_audio.unwrap_or(false);
    let preset = options.preset.as_deref().unwrap_or("fast");
    let tune = options.tune.as_deref();

    let crf = get_quality(quality, &codec_lower);
    let codec_preset = get_codec_preset(preset, &codec_lower);
    let vp9_speed = get_vp9_speed(preset, &codec_lower);
    let is_vp9 = vp9_speed.is_some();

    log::debug!(
        target: "tiny_vid::ffmpeg::builder",
        "Building FFmpeg command: codec={}, CRF={}, preset={}, input={} -> output={}",
        codec_lower,
        crf,
        codec_preset,
        input_path,
        output_path
    );

    let mut args = vec![
        "-nostdin".to_string(),
        "-threads".to_string(),
        "0".to_string(),
        "-thread_queue_size".to_string(),
        "512".to_string(),
        "-progress".to_string(),
        "pipe:1".to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-c:v".to_string(),
        codec,
    ];

    let output_format = options
        .output_format
        .as_deref()
        .unwrap_or("mp4")
        .to_lowercase();
    let is_webm = output_format == "webm";

    if remove_audio {
        args.push("-an".to_string());
    } else if is_webm {
        args.extend([
            "-c:a".to_string(),
            "libopus".to_string(),
            "-b:a".to_string(),
            "128k".to_string(),
        ]);
    } else {
        args.extend([
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            "128k".to_string(),
        ]);
    }

    if scale < 1.0 {
        let scale_filter = format!("scale=round(iw*{}/2)*2:-2", scale);
        args.extend(["-vf".to_string(), scale_filter]);
    }

    if is_vp9 {
        let (deadline, cpu_used) = vp9_speed.unwrap();
        args.extend(["-deadline".to_string(), deadline.to_string()]);
        args.extend(["-cpu-used".to_string(), cpu_used.to_string()]);
        args.extend(["-row-mt".to_string(), "1".to_string()]);
    } else {
        args.extend(["-preset".to_string(), codec_preset]);
    }
    args.extend(["-r".to_string(), fps.to_string()]);
    if !is_webm {
        args.extend(["-movflags".to_string(), "+faststart".to_string()]);
    }

    if codec_lower.contains("svtav1") {
        args.extend(["-pix_fmt".to_string(), "yuv420p".to_string()]);
        args.extend(["-tag:v".to_string(), "av01".to_string()]);
    }
    if codec_lower.contains("x265") || codec_lower.contains("hevc") {
        args.extend(["-tag:v".to_string(), "hvc1".to_string()]);
    }

    if let Some(tune_val) = tune {
        if !tune_val.is_empty()
            && tune_val != "none"
            && !codec_lower.contains("svtav1")
            && !is_vp9
        {
            args.extend(["-tune".to_string(), tune_val.to_string()]);
        }
    }

    if is_vp9 {
        args.extend(["-b:v".to_string(), "0".to_string()]);
    }
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

    args.push(output_path.to_string());
    args
}

/// Formats args for readable display: option and value on the same line when the next arg is a value.
pub fn format_args_for_display_multiline(args: &[String]) -> String {
    if args.is_empty() {
        return String::new();
    }
    let mut lines = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        let line = if arg.starts_with('-')
            && i + 1 < args.len()
            && !args[i + 1].starts_with('-')
        {
            let value = &args[i + 1];
            i += 2;
            format!("  {} {}", arg, value)
        } else {
            i += 1;
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
    fn default_options_produces_expected_args() {
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &opts());
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
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o);
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
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o);
        assert!(args.contains(&"-an".to_string()));
        assert!(!args.iter().any(|a| a == "-c:a"));
    }

    #[test]
    fn h264_quality_maps_to_crf() {
        let mut o = opts();
        o.quality = Some(0);
        o.codec = Some("libx264".to_string());
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o);
        let crf_idx = args.iter().position(|a| a == "-crf").unwrap();
        assert_eq!(args.get(crf_idx + 1).unwrap(), "51");
    }

    #[test]
    fn h265_quality_uses_different_crf_range() {
        let mut o = opts();
        o.quality = Some(100);
        o.codec = Some("libx265".to_string());
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o);
        let crf_idx = args.iter().position(|a| a == "-crf").unwrap();
        assert_eq!(args.get(crf_idx + 1).unwrap(), "28");
    }

    #[test]
    fn h265_adds_hvc1_tag_for_quicktime() {
        let mut o = opts();
        o.codec = Some("libx265".to_string());
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o);
        assert!(args.contains(&"-tag:v".to_string()));
        let tag_idx = args.iter().position(|a| a == "-tag:v").unwrap();
        assert_eq!(args.get(tag_idx + 1).unwrap(), "hvc1");
    }

    #[test]
    fn svtav1_preset_map() {
        let mut o = opts();
        o.codec = Some("libsvtav1".to_string());
        o.preset = Some("ultrafast".to_string());
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o);
        assert!(args.contains(&"-preset".to_string()));
        let preset_idx = args.iter().position(|a| a == "-preset").unwrap();
        assert_eq!(args.get(preset_idx + 1).unwrap(), "12");
    }

    #[test]
    fn max_bitrate_adds_maxrate_and_bufsize() {
        let mut o = opts();
        o.max_bitrate = Some(2000);
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o);
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
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o);
        let crf_idx = args.iter().position(|a| a == "-crf").unwrap();
        assert_eq!(args.get(crf_idx + 1).unwrap(), "63");
        o.quality = Some(100);
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o);
        let crf_idx = args.iter().position(|a| a == "-crf").unwrap();
        assert_eq!(args.get(crf_idx + 1).unwrap(), "24");
    }

    #[test]
    fn quality_75_maps_to_different_crf_per_codec() {
        let mut o = opts();
        o.quality = Some(75);
        o.codec = Some("libx264".to_string());
        let args_h264 = build_ffmpeg_command("/in.mp4", "/out.mp4", &o);
        o.codec = Some("libsvtav1".to_string());
        let args_av1 = build_ffmpeg_command("/in.mp4", "/out.mp4", &o);
        let crf_idx_h264 = args_h264.iter().position(|a| a == "-crf").unwrap();
        let crf_idx_av1 = args_av1.iter().position(|a| a == "-crf").unwrap();
        assert_eq!(args_h264.get(crf_idx_h264 + 1).unwrap(), "30");
        assert_eq!(args_av1.get(crf_idx_av1 + 1).unwrap(), "34");
    }

    #[test]
    fn tune_added_for_x264() {
        let mut o = opts();
        o.tune = Some("film".to_string());
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o);
        assert!(args.contains(&"-tune".to_string()));
        let tune_idx = args.iter().position(|a| a == "-tune").unwrap();
        assert_eq!(args.get(tune_idx + 1).unwrap(), "film");
    }

    #[test]
    fn tune_skipped_for_svtav1() {
        let mut o = opts();
        o.codec = Some("libsvtav1".to_string());
        o.tune = Some("film".to_string());
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o);
        assert!(!args.contains(&"-tune".to_string()));
    }

    #[test]
    fn svtav1_adds_pix_fmt_and_av01_tag() {
        let mut o = opts();
        o.codec = Some("libsvtav1".to_string());
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o);
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
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o);
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
            let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o);
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
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o);
        let preset_idx = args.iter().position(|a| a == "-preset").unwrap();
        assert_eq!(args.get(preset_idx + 1).unwrap(), "8");
    }

    #[test]
    fn custom_fps_passthrough() {
        let mut o = opts();
        o.fps = Some(24);
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o);
        let r_idx = args.iter().position(|a| a == "-r").unwrap();
        assert_eq!(args.get(r_idx + 1).unwrap(), "24");
        o.fps = Some(60);
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o);
        let r_idx = args.iter().position(|a| a == "-r").unwrap();
        assert_eq!(args.get(r_idx + 1).unwrap(), "60");
    }

    #[test]
    fn scale_one_no_vf() {
        let mut o = opts();
        o.scale = Some(1.0);
        let args = build_ffmpeg_command("/in.mp4", "/out.mp4", &o);
        assert!(!args.contains(&"-vf".to_string()));
    }

    #[test]
    fn webm_uses_libopus_no_movflags() {
        let mut o = opts();
        o.codec = Some("libsvtav1".to_string());
        o.output_format = Some("webm".to_string());
        o.remove_audio = Some(false);
        let args = build_ffmpeg_command("/in.mp4", "/out.webm", &o);
        assert!(args.contains(&"libopus".to_string()));
        assert!(!args.contains(&"-movflags".to_string()));
        assert!(args.last() == Some(&"/out.webm".to_string()));
    }

    #[test]
    fn webm_no_audio_uses_an() {
        let mut o = opts();
        o.codec = Some("libsvtav1".to_string());
        o.output_format = Some("webm".to_string());
        o.remove_audio = Some(true);
        let args = build_ffmpeg_command("/in.mp4", "/out.webm", &o);
        assert!(args.contains(&"-an".to_string()));
        assert!(!args.contains(&"-movflags".to_string()));
    }

    #[test]
    fn vp9_uses_deadline_cpu_used_bv0() {
        let mut o = opts();
        o.codec = Some("libvpx-vp9".to_string());
        o.output_format = Some("webm".to_string());
        o.preset = Some("fast".to_string());
        let args = build_ffmpeg_command("/in.mp4", "/out.webm", &o);
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
    fn vp9_quality_maps_to_crf() {
        let mut o = opts();
        o.codec = Some("libvpx-vp9".to_string());
        o.output_format = Some("webm".to_string());
        o.quality = Some(0);
        let args = build_ffmpeg_command("/in.mp4", "/out.webm", &o);
        let crf_idx = args.iter().position(|a| a == "-crf").unwrap();
        assert_eq!(args.get(crf_idx + 1).unwrap(), "63", "quality 0 -> worst CRF");
        o.quality = Some(100);
        let args2 = build_ffmpeg_command("/in.mp4", "/out.webm", &o);
        let crf_idx2 = args2.iter().position(|a| a == "-crf").unwrap();
        assert_eq!(args2.get(crf_idx2 + 1).unwrap(), "20", "quality 100 -> best CRF");
    }
}
