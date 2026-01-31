mod builder;
mod discovery;
mod error;
mod progress;
mod runner;
mod temp;
mod verify;

pub use builder::build_ffmpeg_command;
pub use error::parse_ffmpeg_error;
pub use runner::{run_ffmpeg_blocking, terminate_all_ffmpeg};
pub use temp::{
    cleanup_previous_preview_paths, cleanup_transcode_temp, set_transcode_temp,
    store_preview_paths_for_cleanup, TempFileManager,
};
#[cfg(test)]
pub use verify::verify_video;

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TranscodeOptions {
    pub codec: Option<String>,
    pub quality: Option<u32>,
    pub max_bitrate: Option<u32>,
    pub scale: Option<f64>,
    pub fps: Option<u32>,
    pub remove_audio: Option<bool>,
    pub preset: Option<String>,
    pub tune: Option<String>,
    pub preview_duration: Option<u32>,
}

impl Default for TranscodeOptions {
    fn default() -> Self {
        Self {
            codec: Some("libx264".to_string()),
            quality: Some(75),
            max_bitrate: None,
            scale: Some(1.0),
            fps: Some(30),
            remove_audio: Some(false),
            preset: Some("fast".to_string()),
            tune: None,
            preview_duration: Some(3),
        }
    }
}
