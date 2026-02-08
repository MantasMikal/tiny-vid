export interface FfmpegSizeEstimate {
  bestSize: number;
  lowSize: number;
  highSize: number;
  confidence: "high" | "medium" | "low";
  method: "sampled_bitrate";
  sampleCount: number;
  sampleSecondsTotal: number;
}

export interface FfmpegPreviewResult {
  originalPath: string;
  compressedPath: string;
  /** Start offset (seconds) of original. Delay compressed playback by this to sync. */
  startOffsetSeconds?: number;
  /** Present when includeEstimate was true. */
  estimate?: FfmpegSizeEstimate;
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

export interface AppCapabilitiesResult {
  protocolVersion: 2;
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
  preserveMetadata?: boolean;
  audioBitrate?: number;
  downmixToStereo?: boolean;
  preserveSubtitles?: boolean;
  subtitleStreamCount?: number;
  audioCodecName?: string;
  audioChannels?: number;
}

export type MediaInspectParams =
  | {
      kind: "metadata";
      inputPath: string;
    }
  | {
      kind: "commandPreview";
      inputPath?: string;
      options: TranscodeOptions;
    };

export type MediaInspectResult = GetVideoMetadataResult | string;

export type MediaProcessParams =
  | {
      kind: "preview";
      inputPath: string;
      options: TranscodeOptions;
      previewStartSeconds?: number;
      includeEstimate?: boolean;
    }
  | {
      kind: "transcode";
      inputPath: string;
      options: TranscodeOptions;
    }
  | {
      kind: "commit";
      commitToken: string;
      outputPath: string;
    }
  | {
      kind: "discard";
      commitToken: string;
    };

export interface MediaProcessTranscodeResult {
  jobId: number;
  commitToken: string;
}

export interface MediaProcessCommitResult {
  savedPath: string;
}

export interface MediaProcessDiscardResult {
  discarded: true;
}

export type MediaProcessResult =
  | FfmpegPreviewResult
  | MediaProcessTranscodeResult
  | MediaProcessCommitResult
  | MediaProcessDiscardResult;

export interface MediaCancelParams {
  jobId?: number;
}

export interface MediaCancelResult {
  cancelled: boolean;
  jobId: number | null;
}

export type MediaJobKind = "preview" | "transcode";

export interface MediaJobProgressPayload {
  jobId: number;
  kind: MediaJobKind;
  progress: number;
  step?: string | null;
}

export interface MediaJobErrorPayload {
  jobId: number;
  kind: MediaJobKind;
  summary: string;
  detail: string;
}

export interface MediaJobCompletePayload {
  jobId: number;
  kind: MediaJobKind;
}

export interface NativeInvokeContract {
  "app.capabilities": {
    args: undefined;
    result: AppCapabilitiesResult;
  };
  "media.inspect": {
    args: MediaInspectParams;
    result: MediaInspectResult;
  };
  "media.process": {
    args: MediaProcessParams;
    result: MediaProcessResult;
  };
  "media.cancel": {
    args: MediaCancelParams | undefined;
    result: MediaCancelResult;
  };
  get_pending_opened_files: {
    args: undefined;
    result: string[];
  };
}

export type NativeInvokeCommand = keyof NativeInvokeContract;

export type NativeInvokeArgs<C extends NativeInvokeCommand> = NativeInvokeContract[C]["args"];

export type NativeInvokeResult<C extends NativeInvokeCommand> = NativeInvokeContract[C]["result"];
