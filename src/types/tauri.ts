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
  estimate?: FfmpegSizeEstimate;
}

export interface FfmpegSizeEstimate {
  bestSize: number;
  lowSize: number;
  highSize: number;
  confidence: "high" | "medium" | "low";
  method: "sampled_bitrate";
  sampleCount: number;
  sampleSecondsTotal: number;
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
  subtitleStreamCount?: number;
  audioCodecName?: string;
  audioChannels?: number;
  encoder?: string;
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
  rateControlMode?: "quality" | "targetSize";
  targetSizeMb?: number;
  previewDuration?: number;
  durationSecs?: number;
  preserveAdditionalAudioStreams?: boolean;
  audioStreamCount?: number;
  preserveMetadata?: boolean;
  audioBitrate?: number;
  downmixToStereo?: boolean;
  preserveSubtitles?: boolean;
  subtitleStreamCount?: number;
  audioCodecName?: string;
  audioChannels?: number;
}
