export interface FfmpegErrorPayload {
  summary: string;
  detail: string;
}

export interface FfmpegPreviewResult {
  originalPath: string;
  compressedPath: string;
  /** Start offset (seconds) of original. Delay compressed playback by this to sync. */
  startOffsetSeconds?: number;
  /** Present when includeEstimate was true. */
  estimatedSize?: number;
}

export interface FfmpegProgressPayload {
  progress: number;
  step?: string;
}

export interface GetVideoMetadataResult {
  duration: number;
  width: number;
  height: number;
  size: number;
  sizeMb: number;
  fps: number;
  codecName?: string;
  codecLongName?: string;
  videoBitRate?: number;
  formatBitRate?: number;
  formatName?: string;
  formatLongName?: string;
  nbStreams?: number;
  audioStreamCount: number;
}

export interface CodecInfo {
  value: string;
  name: string;
  formats: string[];
  supportsTune: boolean;
  presetType: string;
}

export interface BuildVariantResult {
  variant: "standalone" | "lgpl";
  codecs: CodecInfo[];
}

export interface TranscodeOptions {
  codec?: string;
  quality?: number;
  maxBitrate?: number;
  scale?: number;
  fps?: number;
  removeAudio?: boolean;
  preset?: string;
  tune?: string;
  outputFormat?: string;
  previewDuration?: number;
  durationSecs?: number;
  preserveAdditionalAudioStreams?: boolean;
  audioStreamCount?: number;
}
