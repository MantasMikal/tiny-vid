import { create } from "zustand";

import {
  type CompressionOptions,
  getDefaultExtension,
} from "@/features/compression/lib/compression-options";
import type { VideoMetadata } from "@/features/compression/lib/get-video-metadata";
import { getVideoMetadataFromPath } from "@/features/compression/lib/get-video-metadata";
import {
  createInitialOptions,
  DEFAULT_FPS,
  DEFAULT_PRESET_ID,
  resolve,
} from "@/features/compression/lib/options-pipeline";
import { type ResultError, tryCatch } from "@/lib/try-catch";
import { formatError } from "@/lib/utils";
import { desktopClient } from "@/platform/desktop/client";
import type {
  CodecInfo,
  FfmpegPreviewResult,
  FfmpegSizeEstimate,
  MediaProcessResult,
  MediaProcessTranscodeResult,
  TranscodeOptions,
} from "@/types/native";

export enum WorkerState {
  Idle = "idle",
  GeneratingPreview = "generating-preview",
  Transcoding = "transcoding",
}

export interface VideoPreview {
  originalSrc: string;
  compressedSrc: string;
  /** Start offset (seconds) of original. Delay compressed playback by this to sync. */
  startOffsetSeconds?: number;
}

function toRustOptions(
  opts: CompressionOptions,
  durationSecs?: number,
  metadata?: Pick<
    VideoMetadata,
    "audioStreamCount" | "subtitleStreamCount" | "audioCodecName" | "audioChannels"
  >
): TranscodeOptions {
  return {
    codec: opts.codec,
    quality: opts.quality,
    maxBitrate: opts.maxBitrate,
    scale: opts.scale,
    fps: opts.fps,
    removeAudio: opts.removeAudio,
    preset: opts.preset,
    tune: opts.tune,
    outputFormat: opts.outputFormat,
    previewDuration: opts.previewDuration ?? 3,
    durationSecs,
    preserveAdditionalAudioStreams: opts.preserveAdditionalAudioStreams ?? false,
    audioStreamCount: metadata?.audioStreamCount,
    preserveMetadata: opts.preserveMetadata ?? false,
    audioBitrate: opts.audioBitrate,
    downmixToStereo: opts.downmixToStereo ?? false,
    preserveSubtitles: opts.preserveSubtitles ?? false,
    subtitleStreamCount: metadata?.subtitleStreamCount,
    audioCodecName: metadata?.audioCodecName,
    audioChannels: metadata?.audioChannels,
  };
}

let debouncePreviewTimer: ReturnType<typeof setTimeout> | null = null;
let debounceScrubPreviewTimer: ReturnType<typeof setTimeout> | null = null;
let previewRequestId = 0;
let commandPreviewRequestId = 0;
let debounceCommandPreviewTimer: ReturnType<typeof setTimeout> | null = null;
let selectPathRequestId = 0;

const SCRUB_PREVIEW_DEBOUNCE_MS = 200;
const OPTIONS_PREVIEW_DEBOUNCE_MS = 300;

async function toPreviewMediaSrc(filePath: string): Promise<string> {
  const assetSrc = await desktopClient.toMediaSrc(filePath);
  return assetSrc;
}

function clampPreviewStartSeconds(
  startSeconds: number,
  duration?: number | null,
  previewDuration?: number | null
) {
  const safeDuration = typeof duration === "number" && Number.isFinite(duration) ? duration : 0;
  const safePreviewDuration =
    typeof previewDuration === "number" && Number.isFinite(previewDuration) ? previewDuration : 0;
  const maxStart = Math.max(0, safeDuration - safePreviewDuration);
  const safeStart = Number.isFinite(startSeconds) ? startSeconds : 0;
  return Math.min(Math.max(0, safeStart), maxStart);
}

function isPreviewProcessResult(result: MediaProcessResult): result is FfmpegPreviewResult {
  return typeof result === "object" && "originalPath" in result && "compressedPath" in result;
}

function isTranscodeProcessResult(
  result: MediaProcessResult
): result is MediaProcessTranscodeResult {
  return typeof result === "object" && "jobId" in result && "commitToken" in result;
}

export interface CompressionState {
  inputPath: string | null;
  videoPreview: VideoPreview | null;
  videoUploading: boolean;
  estimate: FfmpegSizeEstimate | null;
  videoMetadata: VideoMetadata | null;
  previewStartSeconds: number;
  isSaving: boolean;
  compressionOptions: CompressionOptions | null;
  availableCodecs: CodecInfo[];
  initError: string | null;
  error: ResultError | null;
  workerState: WorkerState;
  progress: number;
  /** Step label during preview or transcoding (e.g. "transcode", "preview_extract"). */
  progressStep: string | null;
  activePreviewJobId: number | null;
  activeTranscodeJobId: number | null;
  listenersReady: boolean;
  ffmpegCommandPreview: string | null;
  ffmpegCommandPreviewLoading: boolean;

  initBuildVariant: () => Promise<void>;
  selectPath: (path: string) => Promise<void>;
  browseAndSelectFile: () => Promise<void>;
  transcodeAndSave: () => Promise<void>;
  clear: () => void;
  dismissError: () => void;
  generatePreview: (
    requestId?: number,
    opts?: { includeEstimate?: boolean; previewStartSeconds?: number }
  ) => Promise<void>;
  setPreviewRegionStart: (startSeconds: number) => void;
  setCompressionOptions: (options: CompressionOptions, opts?: { triggerPreview?: boolean }) => void;
  refreshFfmpegCommandPreview: () => Promise<void>;
  terminate: () => Promise<void>;
}

export const useCompressionStore = create<CompressionState>((set, get) => ({
  inputPath: null,
  videoPreview: null,
  videoUploading: false,
  estimate: null,
  videoMetadata: null,
  previewStartSeconds: 0,
  isSaving: false,
  compressionOptions: null,
  availableCodecs: [],
  initError: null,
  error: null,
  workerState: WorkerState.Idle,
  progress: 0,
  progressStep: null,
  activePreviewJobId: null,
  activeTranscodeJobId: null,
  listenersReady: false,
  ffmpegCommandPreview: null,
  ffmpegCommandPreviewLoading: false,

  initBuildVariant: async () => {
    try {
      const result = await desktopClient.invoke("app.capabilities");
      set({
        availableCodecs: result.codecs,
        initError: null,
        compressionOptions: createInitialOptions(result.codecs, DEFAULT_PRESET_ID),
      });
    } catch (error) {
      set({
        availableCodecs: [],
        initError: formatError(error),
      });
    }
  },

  refreshFfmpegCommandPreview: async () => {
    const { compressionOptions, inputPath, videoMetadata } = get();
    if (!compressionOptions) return;
    const requestId = ++commandPreviewRequestId;
    const tid = setTimeout(() => {
      if (commandPreviewRequestId === requestId) {
        set({ ffmpegCommandPreviewLoading: true });
      }
    }, 0);
    try {
      const result = await desktopClient.invoke("media.inspect", {
        kind: "commandPreview",
        options: toRustOptions(compressionOptions, undefined, videoMetadata ?? undefined),
        ...(inputPath ? { inputPath } : {}),
      });
      if (commandPreviewRequestId === requestId) {
        set({ ffmpegCommandPreview: typeof result === "string" ? result : null });
      }
    } catch {
      if (commandPreviewRequestId === requestId) {
        set({ ffmpegCommandPreview: null });
      }
    } finally {
      clearTimeout(tid);
      if (commandPreviewRequestId === requestId) {
        set({ ffmpegCommandPreviewLoading: false });
      }
    }
  },

  selectPath: async (path: string) => {
    const requestId = ++selectPathRequestId;
    const { workerState } = get();
    if (workerState !== WorkerState.Idle) {
      await get().terminate();
    }

    set({
      inputPath: path,
      videoUploading: true,
      videoPreview: null,
      videoMetadata: null,
      estimate: null,
      previewStartSeconds: 0,
      error: null,
    });
    const metadataResult = await tryCatch(() => getVideoMetadataFromPath(path), "Metadata Error");
    if (!metadataResult.ok) {
      if (!metadataResult.aborted) {
        set({
          error: {
            type: "Metadata Error",
            message: "Failed to load video metadata",
            detail: metadataResult.error.detail ?? metadataResult.error.message,
          },
        });
      }
      if (selectPathRequestId === requestId) {
        set({ videoUploading: false });
      }
      return;
    }
    if (selectPathRequestId !== requestId) return;
    set({ videoMetadata: metadataResult.value });

    const { compressionOptions } = get();
    const sourceFps = metadataResult.value.fps;
    if (compressionOptions && sourceFps > 0 && sourceFps < DEFAULT_FPS) {
      get().setCompressionOptions(
        { ...compressionOptions, fps: sourceFps },
        { triggerPreview: false }
      );
    }
    if (!compressionOptions?.generatePreview) {
      set({ estimate: null });
    }

    void get().refreshFfmpegCommandPreview();

    await tryCatch(
      async () => {
        if (selectPathRequestId !== requestId) return;
        const { compressionOptions } = get();
        if (compressionOptions?.generatePreview) {
          const previewStartSeconds = clampPreviewStartSeconds(
            get().previewStartSeconds,
            metadataResult.value.duration,
            compressionOptions.previewDuration
          );
          previewRequestId++;
          await get().generatePreview(previewRequestId, {
            includeEstimate: true,
            previewStartSeconds,
          });
        } else {
          set({ videoUploading: false });
        }
      },
      "Preview Error",
      {
        onFinally: () => {
          if (get().inputPath === path && selectPathRequestId === requestId) {
            set({ videoUploading: false });
          }
        },
      }
    );
  },

  browseAndSelectFile: async () => {
    const selected = await desktopClient.openDialog({
      multiple: false,
      directory: false,
      filters: [
        {
          name: "Video",
          extensions: ["mp4", "mpeg", "webm", "mov", "3gp", "avi", "flv", "mkv", "ogg"],
        },
      ],
    });
    if (selected && typeof selected === "string") {
      await get().selectPath(selected);
    }
  },

  transcodeAndSave: async () => {
    const { inputPath, compressionOptions, videoMetadata } = get();
    if (!inputPath || !compressionOptions) return;

    set({
      workerState: WorkerState.Transcoding,
      progress: 0,
      progressStep: null,
      activeTranscodeJobId: null,
      activePreviewJobId: null,
      error: null,
    });
    const transcodeResult = await tryCatch(
      () =>
        desktopClient.invoke("media.process", {
          kind: "transcode",
          inputPath,
          options: toRustOptions(
            compressionOptions,
            videoMetadata?.duration,
            videoMetadata ?? undefined
          ),
        }),
      "Transcode Error"
    );
    if (!transcodeResult.ok) {
      if (!transcodeResult.aborted) {
        set({
          workerState: WorkerState.Idle,
          error: transcodeResult.error,
        });
      }
      await get().terminate();
      return;
    }
    if (!isTranscodeProcessResult(transcodeResult.value)) {
      set({
        workerState: WorkerState.Idle,
        error: {
          type: "Transcode Error",
          message: "Invalid transcode response",
          detail: "media.process(kind=transcode) returned an unexpected payload",
        },
      });
      await get().terminate();
      return;
    }

    const { commitToken, jobId } = transcodeResult.value;
    set({ workerState: WorkerState.Idle, progress: 1, activeTranscodeJobId: jobId });

    set({ isSaving: true });
    await tryCatch(
      async () => {
        const inputFilename = inputPath.split(/[/\\]/).pop() ?? "output";
        const basename = inputFilename.replace(/\.[^.]+$/, "") || "output";
        const ext = getDefaultExtension(compressionOptions.outputFormat);
        const outputPath = await desktopClient.saveDialog({
          defaultPath: `compressed-${basename}.${ext}`,
          filters: [
            {
              name: "Video",
              extensions: [ext],
            },
          ],
        });

        if (!outputPath) {
          await tryCatch(
            () =>
              desktopClient.invoke("media.process", {
                kind: "discard",
                commitToken,
              }),
            "Cleanup Error"
          );
          return;
        }

        const commitResult = await tryCatch(
          () =>
            desktopClient.invoke("media.process", {
              kind: "commit",
              commitToken,
              outputPath,
            }),
          "Save Error"
        );
        if (!commitResult.ok) {
          if (!commitResult.aborted) {
            set({ error: commitResult.error });
          }
          await tryCatch(
            () =>
              desktopClient.invoke("media.process", {
                kind: "discard",
                commitToken,
              }),
            "Cleanup Error"
          );
        }
      },
      "Save Error",
      {
        onFinally: () => {
          set({ isSaving: false, activeTranscodeJobId: null, progressStep: null });
        },
      }
    );
  },

  clear: () => {
    set({
      inputPath: null,
      videoPreview: null,
      videoMetadata: null,
      estimate: null,
      previewStartSeconds: 0,
      error: null,
      activePreviewJobId: null,
      activeTranscodeJobId: null,
    });
    void get().refreshFfmpegCommandPreview();
  },

  dismissError: () => {
    set({ error: null });
  },

  generatePreview: async (
    requestId?: number,
    opts?: { includeEstimate?: boolean; previewStartSeconds?: number }
  ) => {
    const { inputPath, compressionOptions, workerState } = get();
    if (!inputPath || !compressionOptions) return;

    if (workerState === WorkerState.GeneratingPreview) {
      await get().terminate();
    }

    const includeEstimate = opts?.includeEstimate ?? true;
    const basePreviewState = {
      workerState: WorkerState.GeneratingPreview,
      progress: 0,
      progressStep: null,
      activePreviewJobId: null,
      activeTranscodeJobId: null,
      error: null,
    };
    set(basePreviewState);

    const previewStartSeconds = clampPreviewStartSeconds(
      opts?.previewStartSeconds ?? get().previewStartSeconds,
      get().videoMetadata?.duration,
      compressionOptions.previewDuration
    );

    const result = await tryCatch(
      () =>
        desktopClient.invoke("media.process", {
          kind: "preview",
          inputPath,
          options: toRustOptions(compressionOptions, undefined, get().videoMetadata ?? undefined),
          previewStartSeconds,
          includeEstimate,
        }),
      "Preview Error"
    );

    if (requestId !== undefined && requestId !== previewRequestId) return;

    if (result.ok) {
      if (!isPreviewProcessResult(result.value)) {
        set({
          workerState: WorkerState.Idle,
          error: {
            type: "Preview Error",
            message: "Invalid preview response",
            detail: "media.process(kind=preview) returned an unexpected payload",
          },
          activePreviewJobId: null,
        });
        return;
      }
      const [originalSrc, compressedSrc] = await Promise.all([
        toPreviewMediaSrc(result.value.originalPath),
        toPreviewMediaSrc(result.value.compressedPath),
      ]);
      if (requestId !== undefined && requestId !== previewRequestId) {
        return;
      }
      set({
        previewStartSeconds,
        videoPreview: {
          originalSrc,
          compressedSrc,
          startOffsetSeconds: result.value.startOffsetSeconds,
        },
        ...(result.value.estimate != null && { estimate: result.value.estimate }),
        workerState: WorkerState.Idle,
        progress: 1,
        progressStep: null,
        activePreviewJobId: null,
      });
    } else if (!result.aborted) {
      set({
        workerState: WorkerState.Idle,
        error: result.error,
        activePreviewJobId: null,
      });
      await get().terminate();
    } else {
      set({
        workerState: WorkerState.Idle,
        progress: 0,
        progressStep: null,
        activePreviewJobId: null,
      });
    }
  },

  setCompressionOptions: (options: CompressionOptions, opts?: { triggerPreview?: boolean }) => {
    const { availableCodecs } = get();
    if (availableCodecs.length === 0) return;
    const resolved = resolve(options, availableCodecs);
    set({ compressionOptions: resolved });
    const clampedPreviewStart = clampPreviewStartSeconds(
      get().previewStartSeconds,
      get().videoMetadata?.duration,
      resolved.previewDuration
    );
    if (clampedPreviewStart !== get().previewStartSeconds) {
      set({ previewStartSeconds: clampedPreviewStart });
    }
    if (debounceCommandPreviewTimer) {
      clearTimeout(debounceCommandPreviewTimer);
    }
    debounceCommandPreviewTimer = setTimeout(() => {
      debounceCommandPreviewTimer = null;
      void get().refreshFfmpegCommandPreview();
    }, 250);
    if (opts?.triggerPreview === false) return;
    if (!resolved.generatePreview) return;

    const { inputPath } = get();
    if (!inputPath) return;

    if (debouncePreviewTimer) {
      clearTimeout(debouncePreviewTimer);
    }

    debouncePreviewTimer = setTimeout(() => {
      debouncePreviewTimer = null;
      previewRequestId++;
      void get().generatePreview(previewRequestId, {
        includeEstimate: true,
        previewStartSeconds: clampedPreviewStart,
      });
    }, OPTIONS_PREVIEW_DEBOUNCE_MS);
  },

  setPreviewRegionStart: (startSeconds: number) => {
    const { compressionOptions, inputPath, workerState } = get();
    const clampedStart = clampPreviewStartSeconds(
      startSeconds,
      get().videoMetadata?.duration,
      compressionOptions?.previewDuration
    );
    set({ previewStartSeconds: clampedStart });

    if (!inputPath || !compressionOptions) return;
    if (workerState === WorkerState.Transcoding) return;
    if (!compressionOptions.generatePreview) return;
    if (debounceScrubPreviewTimer) {
      clearTimeout(debounceScrubPreviewTimer);
    }
    debounceScrubPreviewTimer = setTimeout(() => {
      debounceScrubPreviewTimer = null;
      previewRequestId++;
      void get().generatePreview(previewRequestId, {
        includeEstimate: false,
        previewStartSeconds: clampedStart,
      });
    }, SCRUB_PREVIEW_DEBOUNCE_MS);
  },

  terminate: async () => {
    const { workerState, activePreviewJobId, activeTranscodeJobId } = get();
    const activeJobId =
      workerState === WorkerState.GeneratingPreview
        ? activePreviewJobId
        : workerState === WorkerState.Transcoding
          ? activeTranscodeJobId
          : null;

    await tryCatch(
      () =>
        activeJobId == null
          ? desktopClient.invoke("media.cancel")
          : desktopClient.invoke("media.cancel", { jobId: activeJobId }),
      "Terminate Error"
    );
    set({
      workerState: WorkerState.Idle,
      progress: 0,
      progressStep: null,
      activePreviewJobId: null,
      activeTranscodeJobId: null,
    });
  },
}));

export const getCompressionState = () => useCompressionStore.getState();
