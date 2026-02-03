import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
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
import type {
  BuildVariantResult,
  CodecInfo,
  FfmpegPreviewResult,
  TranscodeOptions,
} from "@/types/tauri";

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
  durationSecs?: number
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
  };
}

let debouncePreviewTimer: ReturnType<typeof setTimeout> | null = null;
let previewRequestId = 0;
let commandPreviewRequestId = 0;
let debounceCommandPreviewTimer: ReturnType<typeof setTimeout> | null = null;

interface CompressionState {
  inputPath: string | null;
  videoPreview: VideoPreview | null;
  videoUploading: boolean;
  estimatedSize: number | null;
  videoMetadata: VideoMetadata | null;
  isSaving: boolean;
  compressionOptions: CompressionOptions | null;
  availableCodecs: CodecInfo[];
  initError: string | null;
  error: ResultError | null;
  workerState: WorkerState;
  progress: number;
  listenersReady: boolean;
  ffmpegCommandPreview: string | null;
  ffmpegCommandPreviewLoading: boolean;

  initBuildVariant: () => Promise<void>;
  selectPath: (path: string) => Promise<void>;
  browseAndSelectFile: () => Promise<void>;
  transcodeAndSave: () => Promise<void>;
  clear: () => void;
  dismissError: () => void;
  generatePreview: (requestId?: number) => Promise<void>;
  setCompressionOptions: (
    options: CompressionOptions,
    opts?: { triggerPreview?: boolean }
  ) => void;
  refreshFfmpegCommandPreview: () => Promise<void>;
  terminate: () => Promise<void>;
}

export const useCompressionStore = create<CompressionState>((set, get) => ({
  inputPath: null,
  videoPreview: null,
  videoUploading: false,
  estimatedSize: null,
  videoMetadata: null,
  isSaving: false,
  compressionOptions: null,
  availableCodecs: [],
  initError: null,
  error: null,
  workerState: WorkerState.Idle,
  progress: 0,
  listenersReady: false,
  ffmpegCommandPreview: null,
  ffmpegCommandPreviewLoading: false,

  initBuildVariant: async () => {
    try {
      const result = await invoke<BuildVariantResult>("get_build_variant");
      set({
        availableCodecs: result.codecs,
        initError: null,
        compressionOptions: createInitialOptions(
          result.codecs,
          DEFAULT_PRESET_ID
        ),
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
    const { compressionOptions, inputPath } = get();
    if (!compressionOptions) return;
    const requestId = ++commandPreviewRequestId;
    const tid = setTimeout(() => {
      if (commandPreviewRequestId === requestId) {
        set({ ffmpegCommandPreviewLoading: true });
      }
    }, 0);
    try {
      const result = await invoke<string>("preview_ffmpeg_command", {
        options: toRustOptions(compressionOptions),
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
    const { workerState } = get();
    if (workerState === WorkerState.GeneratingPreview) {
      await get().terminate();
    }

    set({
      inputPath: path,
      videoUploading: true,
      videoPreview: null,
    });
    const metadataResult = await tryCatch(
      () => getVideoMetadataFromPath(path),
      "Metadata Error"
    );
    if (!metadataResult.ok) {
      if (!metadataResult.aborted) {
        set({ error: metadataResult.error });
      }
      set({ videoUploading: false });
      return;
    }
    set({ videoMetadata: metadataResult.value });

    const { compressionOptions } = get();
    const sourceFps = metadataResult.value.fps;
    if (compressionOptions && sourceFps > 0 && sourceFps < DEFAULT_FPS) {
      get().setCompressionOptions(
        { ...compressionOptions, fps: sourceFps },
        { triggerPreview: false }
      );
    }

    void get().refreshFfmpegCommandPreview();

    await tryCatch(
      async () => {
        const { compressionOptions } = get();
        if (compressionOptions?.generatePreview) {
          set({ workerState: WorkerState.GeneratingPreview, error: null });
          const result = await tryCatch(
            () =>
              invoke<FfmpegPreviewResult>("ffmpeg_preview", {
                inputPath: path,
                options: toRustOptions(compressionOptions),
              }),
            "Preview Error"
          );

          if (result.ok) {
            set({
              estimatedSize: result.value.estimatedSize,
              videoPreview: {
                originalSrc: convertFileSrc(result.value.originalPath),
                compressedSrc: convertFileSrc(result.value.compressedPath),
                startOffsetSeconds: result.value.startOffsetSeconds,
              },
              workerState: WorkerState.Idle,
            });
          } else if (!result.aborted) {
            set({
              workerState: WorkerState.Idle,
              error: result.error,
            });
            void get().terminate();
          }
        }
      },
      "Preview Error",
      {
        onFinally: () => {
          if (get().inputPath === path) {
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
          extensions: [
            "mp4",
            "mpeg",
            "webm",
            "mov",
            "3gp",
            "avi",
            "flv",
            "mkv",
            "ogg",
          ],
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

    set({ workerState: WorkerState.Transcoding, progress: 0, error: null });
    const transcodeResult = await tryCatch(
      () =>
        invoke<string>("ffmpeg_transcode_to_temp", {
          inputPath,
          options: toRustOptions(compressionOptions, videoMetadata?.duration),
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
          await tryCatch(
            () => invoke("cleanup_temp_file", { path: tempPath }),
            "Cleanup Error"
          );
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
          await tryCatch(
            () => invoke("cleanup_temp_file", { path: tempPath }),
            "Cleanup Error"
          );
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
    set({
      inputPath: null,
      videoPreview: null,
      videoMetadata: null,
      estimatedSize: null,
      error: null,
    });
    void get().refreshFfmpegCommandPreview();
  },

  dismissError: () => {
    set({ error: null });
  },

  generatePreview: async (requestId?: number) => {
    const { inputPath, compressionOptions, workerState } = get();
    if (!inputPath || !compressionOptions) return;

    if (workerState === WorkerState.GeneratingPreview) {
      await get().terminate();
    }

    set({ workerState: WorkerState.GeneratingPreview, error: null });

    const result = await tryCatch(
      () =>
        invoke<FfmpegPreviewResult>("ffmpeg_preview", {
          inputPath,
          options: toRustOptions(compressionOptions),
        }),
      "Preview Error"
    );

    if (requestId !== undefined && requestId !== previewRequestId) return;

    if (result.ok) {
      set({
        estimatedSize: result.value.estimatedSize,
        videoPreview: {
          originalSrc: convertFileSrc(result.value.originalPath),
          compressedSrc: convertFileSrc(result.value.compressedPath),
          startOffsetSeconds: result.value.startOffsetSeconds,
        },
        workerState: WorkerState.Idle,
      });
    } else if (!result.aborted) {
      set({
        workerState: WorkerState.Idle,
        error: result.error,
      });
      await get().terminate();
    }
  },

  setCompressionOptions: (
    options: CompressionOptions,
    opts?: { triggerPreview?: boolean }
  ) => {
    const { availableCodecs } = get();
    if (availableCodecs.length === 0) return;
    const resolved = resolve(options, availableCodecs);
    set({ compressionOptions: resolved });
    if (debounceCommandPreviewTimer) {
      clearTimeout(debounceCommandPreviewTimer);
    }
    debounceCommandPreviewTimer = setTimeout(() => {
      debounceCommandPreviewTimer = null;
      void get().refreshFfmpegCommandPreview();
    }, 250);
    if (opts?.triggerPreview === false) return;
    if (!options.generatePreview) return;

    const { inputPath } = get();
    if (!inputPath) return;

    if (debouncePreviewTimer) {
      clearTimeout(debouncePreviewTimer);
    }

    debouncePreviewTimer = setTimeout(() => {
      debouncePreviewTimer = null;
      previewRequestId++;
      void get().generatePreview(previewRequestId);
    }, 300);
  },

  terminate: async () => {
    await tryCatch(() => invoke("ffmpeg_terminate"), "Terminate Error");
    set({
      workerState: WorkerState.Idle,
      progress: 0,
    });
  },
}));
