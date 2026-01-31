/**
 * Types for Tauri IPC - must stay in sync with Rust backend.
 *
 * FfmpegErrorPayload: mirrors ffmpeg::error::FfmpegErrorPayload
 * FfmpegPreviewResult: mirrors lib::PreviewResult
 * TranscodeOptions: mirrors ffmpeg::TranscodeOptions (options for ffmpeg_transcode_to_temp and ffmpeg_preview)
 */

export interface FfmpegErrorPayload {
  summary: string;
  detail: string;
}

export interface FfmpegPreviewResult {
  originalPath: string;
  compressedPath: string;
  estimatedSize: number;
}

/** Options for ffmpeg_transcode_to_temp and ffmpeg_preview - mirrors ffmpeg::TranscodeOptions */
export interface TranscodeOptions {
  codec?: string;
  quality?: number;
  maxBitrate?: number;
  scale?: number;
  fps?: number;
  removeAudio?: boolean;
  preset?: string;
  tune?: string;
  previewDuration?: number;
  durationSecs?: number;
}
