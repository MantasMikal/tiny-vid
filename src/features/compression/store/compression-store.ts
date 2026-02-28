import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { platform } from "@tauri-apps/plugin-os";
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
import { getTargetSizeStatus } from "@/features/compression/lib/target-size";
import { type ResultError, tryCatch } from "@/lib/try-catch";
import type {
  BuildVariantResult,
  CodecInfo,
  FfmpegPreviewResult,
  FfmpegSizeEstimate,
  TranscodeOptions,
} from "@/types/tauri";

export enum WorkerState {
  Idle = "idle",
  GeneratingPreview = "generating-preview",
  Transcoding = "transcoding",
  ExtractingFrame = "extracting-frame",
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
    rateControlMode: opts.rateControlMode,
    targetSizeMb: opts.targetSizeMb,
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
let activePreviewBlobUrls: string[] = [];

const SCRUB_PREVIEW_DEBOUNCE_MS = 200;
const OPTIONS_PREVIEW_DEBOUNCE_MS = 300;
const runningInTauri = isTauri();
const isLinuxWebview = runningInTauri && platform() === "linux";

function revokeObjectUrls(urls: string[]) {
  for (const url of urls) {
    URL.revokeObjectURL(url);
  }
}

function releaseActivePreviewBlobUrls() {
  revokeObjectUrls(activePreviewBlobUrls);
  activePreviewBlobUrls = [];
}

async function toPreviewMediaSrc(filePath: string, nextBlobUrls: string[]): Promise<string> {
  if (!runningInTauri) {
    return filePath;
  }
  const assetSrc = convertFileSrc(filePath);
  if (!isLinuxWebview) {
    return assetSrc;
  }
  try {
    const bytes = await invoke<number[]>("preview_media_bytes", { path: filePath });
    const blobUrl = URL.createObjectURL(new Blob([Uint8Array.from(bytes)], { type: "video/mp4" }));
    nextBlobUrls.push(blobUrl);
    return blobUrl;
  } catch {
    return assetSrc;
  }
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

function getTargetSizeError(
  options: CompressionOptions | null,
  metadata: VideoMetadata | null
): string | null {
  if (options?.rateControlMode !== "targetSize") return null;
  const status = getTargetSizeStatus({
    rateControlMode: options.rateControlMode,
    targetSizeMb: options.targetSizeMb,
    durationSecs: metadata?.duration,
    removeAudio: options.removeAudio,
    audioBitrateKbps: options.audioBitrate,
    audioStreamCount: metadata?.audioStreamCount,
    preserveAdditionalAudioStreams: options.preserveAdditionalAudioStreams,
    requireDuration: true,
  });
  return status.error;
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
  listenersReady: boolean;
  ffmpegCommandPreview: string | null;
  ffmpegCommandPreviewLoading: boolean;

  initBuildVariant: () => Promise<void>;
  selectPath: (path: string) => Promise<void>;
  browseAndSelectFile: () => Promise<void>;
  transcodeAndSave: () => Promise<void>;
  extractFirstFrame: () => Promise<void>;
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
  listenersReady: false,
  ffmpegCommandPreview: null,
  ffmpegCommandPreviewLoading: false,

  initBuildVariant: async () => {
    try {
      const result = await invoke<BuildVariantResult>("get_build_variant");
      set({
        availableCodecs: result.codecs,
        initError: null,
        compressionOptions: createInitialOptions(result.codecs, DEFAULT_PRESET_ID),
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      set({
        availableCodecs: [],
        initError: message,
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
      const result = await invoke<string>("preview_ffmpeg_command", {
        options: toRustOptions(
          compressionOptions,
          videoMetadata?.duration,
          videoMetadata ?? undefined
        ),
        inputPath,
      });
      if (commandPreviewRequestId === requestId) {
        set({ ffmpegCommandPreview: result });
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
    releaseActivePreviewBlobUrls();

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
    if (
      !compressionOptions?.generatePreview ||
      compressionOptions.rateControlMode === "targetSize"
    ) {
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
            includeEstimate: compressionOptions.rateControlMode !== "targetSize",
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
    const selected = await open({
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
    const targetSizeError = getTargetSizeError(compressionOptions, videoMetadata);
    if (targetSizeError) {
      set({
        error: {
          type: "Target Size Error",
          message: targetSizeError,
          detail: targetSizeError,
        },
      });
      return;
    }

    set({ workerState: WorkerState.Transcoding, progress: 0, error: null });
    const transcodeResult = await tryCatch(
      () =>
        invoke<string>("ffmpeg_transcode_to_temp", {
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
    const tempPath = transcodeResult.value;
    set({ workerState: WorkerState.Idle, progress: 1 });

    set({ isSaving: true });
    await tryCatch(
      async () => {
        const inputFilename = inputPath.split(/[/\\]/).pop() ?? "output";
        const basename = inputFilename.replace(/\.[^.]+$/, "") || "output";
        const ext = getDefaultExtension(compressionOptions.outputFormat);
        const outputPath = await save({
          defaultPath: `compressed-${basename}.${ext}`,
          filters: [
            {
              name: "Video",
              extensions: [ext],
            },
          ],
        });

        if (!outputPath) {
          await tryCatch(() => invoke("cleanup_temp_file", { path: tempPath }), "Cleanup Error");
          return;
        }

        const moveResult = await tryCatch(
          () =>
            invoke("move_compressed_file", {
              source: tempPath,
              dest: outputPath,
            }),
          "Save Error"
        );
        if (!moveResult.ok) {
          if (!moveResult.aborted) {
            set({ error: moveResult.error });
          }
          await tryCatch(() => invoke("cleanup_temp_file", { path: tempPath }), "Cleanup Error");
        }
      },
      "Save Error",
      {
        onFinally: () => {
          set({ isSaving: false });
        },
      }
    );
  },

  extractFirstFrame: async () => {
    const { inputPath, compressionOptions } = get();
    if (!inputPath || !compressionOptions) return;

    set({ workerState: WorkerState.ExtractingFrame, error: null });
    const extractResult = await tryCatch(
      () =>
        invoke<string>("extract_first_frame", {
          inputPath,
          quality: compressionOptions.quality,
          scale: compressionOptions.scale,
        }),
      "Extract Frame Error"
    );
    if (!extractResult.ok) {
      if (!extractResult.aborted) {
        set({
          workerState: WorkerState.Idle,
          error: extractResult.error,
        });
      }
      return;
    }
    const tempPath = extractResult.value;
    set({ workerState: WorkerState.Idle });

    set({ isSaving: true });
    await tryCatch(
      async () => {
        const inputFilename = inputPath.split(/[/\\]/).pop() ?? "output";
        const basename = inputFilename.replace(/\.[^.]+$/, "") || "output";
        const outputPath = await save({
          defaultPath: `${basename}-poster.jpg`,
          filters: [{ name: "JPEG Image", extensions: ["jpg"] }],
        });

        if (!outputPath) {
          await tryCatch(() => invoke("cleanup_temp_file", { path: tempPath }), "Cleanup Error");
          return;
        }

        const moveResult = await tryCatch(
          () =>
            invoke("move_compressed_file", {
              source: tempPath,
              dest: outputPath,
            }),
          "Save Error"
        );
        if (!moveResult.ok) {
          if (!moveResult.aborted) {
            set({ error: moveResult.error });
          }
          await tryCatch(() => invoke("cleanup_temp_file", { path: tempPath }), "Cleanup Error");
        }
      },
      "Save Error",
      {
        onFinally: () => {
          set({ isSaving: false });
        },
      }
    );
  },

  clear: () => {
    releaseActivePreviewBlobUrls();
    set({
      inputPath: null,
      videoPreview: null,
      videoMetadata: null,
      estimate: null,
      previewStartSeconds: 0,
      error: null,
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

    const includeEstimate =
      (opts?.includeEstimate ?? true) && compressionOptions.rateControlMode !== "targetSize";
    const basePreviewState = {
      workerState: WorkerState.GeneratingPreview,
      progress: 0,
      progressStep: null,
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
        invoke<FfmpegPreviewResult>("ffmpeg_preview", {
          inputPath,
          options: toRustOptions(
            compressionOptions,
            get().videoMetadata?.duration,
            get().videoMetadata ?? undefined
          ),
          previewStartSeconds,
          includeEstimate,
        }),
      "Preview Error"
    );

    if (requestId !== undefined && requestId !== previewRequestId) return;

    if (result.ok) {
      const nextBlobUrls: string[] = [];
      const [originalSrc, compressedSrc] = await Promise.all([
        toPreviewMediaSrc(result.value.originalPath, nextBlobUrls),
        toPreviewMediaSrc(result.value.compressedPath, nextBlobUrls),
      ]);
      if (requestId !== undefined && requestId !== previewRequestId) {
        revokeObjectUrls(nextBlobUrls);
        return;
      }
      releaseActivePreviewBlobUrls();
      activePreviewBlobUrls = nextBlobUrls;
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
      });
    } else if (!result.aborted) {
      set({
        workerState: WorkerState.Idle,
        error: result.error,
      });
      await get().terminate();
    } else {
      set({
        workerState: WorkerState.Idle,
        progress: 0,
        progressStep: null,
      });
    }
  },

  setCompressionOptions: (options: CompressionOptions, opts?: { triggerPreview?: boolean }) => {
    const { availableCodecs } = get();
    if (availableCodecs.length === 0) return;
    const resolved = resolve(options, availableCodecs);
    const shouldClearEstimate = resolved.rateControlMode === "targetSize";
    set({ compressionOptions: resolved, ...(shouldClearEstimate && { estimate: null }) });
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
        includeEstimate: resolved.rateControlMode !== "targetSize",
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
    await tryCatch(() => invoke("ffmpeg_terminate"), "Terminate Error");
    set({
      workerState: WorkerState.Idle,
      progress: 0,
      progressStep: null,
    });
  },
}));

export const getCompressionState = () => useCompressionStore.getState();
