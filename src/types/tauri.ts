export interface FfmpegErrorPayload {
  summary: string;
  detail: string;
}

export interface FfmpegPreviewResult {
  originalPath: string;
  compressedPath: string;
  estimatedSize: number;
}

export interface GetVideoMetadataResult {
  duration: number;
  width: number;
  height: number;
  size: number;
  sizeMb: number;
}

export interface CodecInfo {
  value: string;
  name: string;
  formats: string[];
  supportsTune: boolean;
  presetType: string;
}

export interface BuildVariantResult {
  variant: "full" | "lgpl-macos";
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
}
