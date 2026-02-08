#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use tiny_vid_core::ffmpeg::ffprobe::get_video_metadata_impl;
use tiny_vid_core::ffmpeg::{
    TranscodeOptions, build_ffmpeg_command, run_ffmpeg_blocking, verify_video,
};

fn block_on_async<T>(
    future: impl std::future::Future<Output = Result<T, tiny_vid_core::error::AppError>>,
) -> Result<T, tiny_vid_core::error::AppError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("create tokio runtime");
    runtime.block_on(future)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecContract {
    IntegrationSmoke,
    IntegrationContract,
}

impl CodecContract {
    fn as_str(self) -> &'static str {
        match self {
            Self::IntegrationSmoke => "integration-smoke",
            Self::IntegrationContract => "integration-contract",
        }
    }
}

pub fn assert_codec_contract(contract: CodecContract) {
    let ffmpeg_path = tiny_vid_core::ffmpeg::discovery::get_ffmpeg_path().unwrap_or_else(|e| {
        panic!(
            "Codec contract `{}` failed.\nFailed to resolve FFmpeg path: {}",
            contract.as_str(),
            e
        )
    });
    let ffmpeg_path_display = ffmpeg_path.display().to_string();

    if cfg!(feature = "lgpl")
        && contract == CodecContract::IntegrationContract
        && !cfg!(target_os = "macos")
    {
        panic!(
            "Codec contract `{}` failed.\nFFmpeg path: {}\nLGPL integration-contract requires macOS.",
            contract.as_str(),
            ffmpeg_path_display
        );
    }

    let mut available =
        tiny_vid_core::ffmpeg::discovery::get_available_codecs().unwrap_or_else(|e| {
            panic!(
                "Codec contract `{}` failed.\nFFmpeg path: {}\nFailed to detect codecs: {}",
                contract.as_str(),
                ffmpeg_path_display,
                e
            )
        });
    available.sort();

    let mut required = required_codecs(contract);
    required.sort();

    let missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|codec| !available.iter().any(|c| c == codec))
        .collect();

    assert!(
        missing.is_empty(),
        "Codec contract `{}` failed.\nFFmpeg path: {}\nMissing codecs: {:?}\nAvailable codecs: {:?}",
        contract.as_str(),
        ffmpeg_path_display,
        missing,
        available
    );
}

fn required_codecs(contract: CodecContract) -> Vec<&'static str> {
    #[cfg(feature = "lgpl")]
    {
        match contract {
            CodecContract::IntegrationSmoke => vec!["h264_videotoolbox"],
            CodecContract::IntegrationContract => vec![
                "h264_videotoolbox",
                "hevc_videotoolbox",
                "libsvtav1",
                "libvpx-vp9",
            ],
        }
    }
    #[cfg(not(feature = "lgpl"))]
    {
        match contract {
            CodecContract::IntegrationSmoke => vec!["libx264"],
            CodecContract::IntegrationContract => {
                vec!["libx264", "libx265", "libsvtav1", "libvpx-vp9"]
            }
        }
    }
}

pub fn default_codec() -> String {
    if cfg!(feature = "lgpl") {
        "h264_videotoolbox".into()
    } else {
        "libx264".into()
    }
}

pub fn opts_with(overrides: impl FnOnce(&mut TranscodeOptions)) -> TranscodeOptions {
    let mut options = TranscodeOptions::default();
    overrides(&mut options);
    options
}

pub fn preview_options(preview_duration: u32) -> TranscodeOptions {
    opts_with(|options| {
        options.codec = Some(default_codec());
        options.remove_audio = Some(true);
        options.preset = Some("ultrafast".into());
        options.preview_duration = Some(preview_duration);
    })
}

pub enum VideoKind {
    Plain,
    MultiAudio(u32),
    Subtitles,
    SubtitlesNoAudio,
}

pub struct IntegrationEnv {
    pub ffmpeg: PathBuf,
    dir: tempfile::TempDir,
}

impl IntegrationEnv {
    pub fn new() -> Self {
        let ffmpeg = tiny_vid_core::ffmpeg::discovery::get_ffmpeg_path()
            .expect("FFmpeg not found")
            .to_path_buf();
        let dir = tempfile::tempdir().expect("tempdir");
        Self { ffmpeg, dir }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    pub fn with_test_video(
        &self,
        input_name: &str,
        duration_secs: f32,
        kind: VideoKind,
    ) -> PathBuf {
        let output_path = self.path(input_name);
        let status = match kind {
            VideoKind::Plain => create_test_video(&self.ffmpeg, &output_path, duration_secs),
            VideoKind::MultiAudio(n) => {
                create_test_video_with_multi_audio(&self.ffmpeg, &output_path, duration_secs, n)
            }
            VideoKind::Subtitles => {
                create_test_video_with_subtitles(&self.ffmpeg, &output_path, duration_secs)
            }
            VideoKind::SubtitlesNoAudio => {
                create_test_video_with_subtitles_no_audio(&self.ffmpeg, &output_path, duration_secs)
            }
        };
        let status = status.expect("failed to create test video");
        assert!(status.success(), "ffmpeg failed to create test video");
        output_path
    }
}

pub fn run_transcode_and_verify(
    input_path: &Path,
    output_path: &Path,
    options: &TranscodeOptions,
    duration_secs: Option<f64>,
) -> Result<(), String> {
    let args = build_ffmpeg_command(
        input_path.to_string_lossy().as_ref(),
        output_path.to_string_lossy().as_ref(),
        options,
        None,
        None,
        None,
    )
    .map_err(|e| e.to_string())?;

    run_ffmpeg_blocking(args, duration_secs, None, None)
        .map_err(|e| format!("run_ffmpeg_blocking failed: {:?}", e))?;

    if !output_path.exists() {
        return Err("output path does not exist".into());
    }
    if fs::metadata(output_path).map_err(|e| e.to_string())?.len() == 0 {
        return Err("output file is empty".into());
    }

    verify_video(output_path, options.codec.as_deref())
        .map_err(|e| format!("Encoded video failed verification: {}", e))
}

pub fn run_preview_and_assert_exists(
    input_path: &Path,
    options: &TranscodeOptions,
    preview_start_seconds: Option<f64>,
) -> tiny_vid_core::test_support::PreviewResultForTest {
    let result = block_on_async(tiny_vid_core::test_support::run_preview_for_test(
        input_path,
        options,
        preview_start_seconds,
    ))
    .expect("run_preview_for_test");
    assert!(Path::new(&result.original_path).exists());
    assert!(Path::new(&result.compressed_path).exists());
    result
}

pub fn run_preview_with_meta_codec_override_and_assert_exists(
    input_path: &Path,
    options: &TranscodeOptions,
    preview_start_seconds: Option<f64>,
    source_codec_override: &str,
) -> tiny_vid_core::test_support::PreviewResultForTest {
    let result = block_on_async(
        tiny_vid_core::test_support::run_preview_for_test_with_meta_codec_override(
            input_path,
            options,
            preview_start_seconds,
            source_codec_override,
        ),
    )
    .expect("run_preview_for_test_with_meta_codec_override");
    assert!(Path::new(&result.original_path).exists());
    assert!(Path::new(&result.compressed_path).exists());
    result
}

pub fn run_preview_with_estimate_and_assert(
    input_path: &Path,
    options: &TranscodeOptions,
    preview_start_seconds: Option<f64>,
) -> tiny_vid_core::test_support::PreviewWithEstimateResultForTest {
    let result = block_on_async(
        tiny_vid_core::test_support::run_preview_with_estimate_for_test(
            input_path,
            options,
            preview_start_seconds,
        ),
    )
    .expect("run_preview_with_estimate_for_test");
    assert!(Path::new(&result.preview.original_path).exists());
    assert!(Path::new(&result.preview.compressed_path).exists());
    result
}

pub fn create_test_video(
    ffmpeg: &Path,
    output_path: &Path,
    duration_secs: f32,
) -> std::io::Result<ExitStatus> {
    let duration_arg = format!("{}", duration_secs);
    #[cfg(not(feature = "lgpl"))]
    {
        Command::new(ffmpeg)
            .args([
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                &format!("testsrc=duration={}:size=320x240:rate=30", duration_arg),
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                output_path.to_string_lossy().as_ref(),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    }
    #[cfg(feature = "lgpl")]
    {
        Command::new(ffmpeg)
            .args([
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                &format!("testsrc=duration={}:size=320x240:rate=30", duration_arg),
                "-c:v",
                "h264_videotoolbox",
                "-allow_sw",
                "1",
                "-q:v",
                "25",
                output_path.to_string_lossy().as_ref(),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    }
}

pub fn create_test_video_with_multi_audio(
    ffmpeg: &Path,
    output_path: &Path,
    duration_secs: f32,
    audio_track_count: u32,
) -> std::io::Result<ExitStatus> {
    if audio_track_count == 0 {
        return create_test_video(ffmpeg, output_path, duration_secs);
    }

    let duration_arg = format!("{}", duration_secs);
    let mut args = vec![
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        "-f".to_string(),
        "lavfi".to_string(),
        "-i".to_string(),
        format!("testsrc=duration={}:size=320x240:rate=30", duration_arg),
    ];

    for i in 0..audio_track_count {
        let freq = 440 + (i as i32) * 220;
        args.push("-f".to_string());
        args.push("lavfi".to_string());
        args.push("-i".to_string());
        args.push(format!("sine=frequency={}:duration={}", freq, duration_arg));
    }

    args.push("-map".to_string());
    args.push("0:v".to_string());
    for i in 0..audio_track_count {
        args.push("-map".to_string());
        args.push(format!("{}:a", i + 1));
    }

    args.push("-c:v".to_string());
    #[cfg(not(feature = "lgpl"))]
    {
        args.push("libx264".to_string());
        args.push("-pix_fmt".to_string());
        args.push("yuv420p".to_string());
    }
    #[cfg(feature = "lgpl")]
    {
        args.push("h264_videotoolbox".to_string());
        args.push("-allow_sw".to_string());
        args.push("1".to_string());
        args.push("-q:v".to_string());
        args.push("25".to_string());
    }
    args.push("-c:a".to_string());
    args.push("aac".to_string());
    args.push("-shortest".to_string());
    args.push(output_path.to_string_lossy().to_string());

    Command::new(ffmpeg)
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
}

pub fn create_test_video_with_subtitles(
    ffmpeg: &Path,
    output_path: &Path,
    duration_secs: f32,
) -> std::io::Result<ExitStatus> {
    let srt_path = output_path
        .parent()
        .unwrap_or_else(|| output_path.as_ref())
        .join("test_subs.srt");
    let srt_content = format!(
        "1\n00:00:00,000 --> 00:00:{:02},000\nTest subtitle\n",
        duration_secs.ceil() as u32
    );
    fs::write(&srt_path, srt_content)?;

    let duration_arg = format!("{}", duration_secs);
    let mut args = vec![
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        "-f".to_string(),
        "lavfi".to_string(),
        "-i".to_string(),
        format!("testsrc=duration={}:size=320x240:rate=30", duration_arg),
        "-f".to_string(),
        "lavfi".to_string(),
        "-i".to_string(),
        format!("sine=frequency=440:duration={}", duration_arg),
        "-f".to_string(),
        "srt".to_string(),
        "-i".to_string(),
        srt_path.to_string_lossy().to_string(),
        "-map".to_string(),
        "0:v".to_string(),
        "-map".to_string(),
        "1:a".to_string(),
        "-map".to_string(),
        "2:s".to_string(),
        "-c:v".to_string(),
    ];

    #[cfg(not(feature = "lgpl"))]
    {
        args.push("libx264".to_string());
        args.push("-pix_fmt".to_string());
        args.push("yuv420p".to_string());
    }
    #[cfg(feature = "lgpl")]
    {
        args.push("h264_videotoolbox".to_string());
        args.push("-allow_sw".to_string());
        args.push("1".to_string());
        args.push("-q:v".to_string());
        args.push("25".to_string());
    }
    args.push("-c:a".to_string());
    args.push("aac".to_string());
    args.push("-c:s".to_string());
    args.push("mov_text".to_string());
    args.push("-shortest".to_string());
    args.push(output_path.to_string_lossy().to_string());

    let result = Command::new(ffmpeg)
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = fs::remove_file(srt_path);
    result
}

pub fn create_test_video_with_subtitles_no_audio(
    ffmpeg: &Path,
    output_path: &Path,
    duration_secs: f32,
) -> std::io::Result<ExitStatus> {
    let srt_path = output_path
        .parent()
        .unwrap_or_else(|| output_path.as_ref())
        .join("test_subs.srt");
    let srt_content = format!(
        "1\n00:00:00,000 --> 00:00:{:02},000\nTest subtitle\n",
        duration_secs.ceil() as u32
    );
    fs::write(&srt_path, srt_content)?;

    let duration_arg = format!("{}", duration_secs);
    let mut args = vec![
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        "-f".to_string(),
        "lavfi".to_string(),
        "-i".to_string(),
        format!("testsrc=duration={}:size=320x240:rate=30", duration_arg),
        "-f".to_string(),
        "srt".to_string(),
        "-i".to_string(),
        srt_path.to_string_lossy().to_string(),
        "-map".to_string(),
        "0:v".to_string(),
        "-map".to_string(),
        "1:s".to_string(),
        "-c:v".to_string(),
    ];

    #[cfg(not(feature = "lgpl"))]
    {
        args.push("libx264".to_string());
        args.push("-pix_fmt".to_string());
        args.push("yuv420p".to_string());
    }
    #[cfg(feature = "lgpl")]
    {
        args.push("h264_videotoolbox".to_string());
        args.push("-allow_sw".to_string());
        args.push("1".to_string());
        args.push("-q:v".to_string());
        args.push("25".to_string());
    }
    args.push("-c:s".to_string());
    args.push("mov_text".to_string());
    args.push("-shortest".to_string());
    args.push(output_path.to_string_lossy().to_string());

    let result = Command::new(ffmpeg)
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = fs::remove_file(srt_path);
    result
}

pub fn metadata(path: &Path) -> tiny_vid_core::ffmpeg::ffprobe::VideoMetadata {
    get_video_metadata_impl(path).expect("get_video_metadata_impl")
}
