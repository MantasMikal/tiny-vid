import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { create } from "zustand";

import {
  type CompressionOptions,
  getCodecsForProfile,
  getDefaultExtension,
  getOutputFormatsForProfile,
  type LicenseProfile,
  resolveOptions,
} from "@/features/compression/lib/compression-options";
import type { VideoMetadata } from "@/features/compression/lib/get-video-metadata";
import { getVideoMetadataFromPath } from "@/features/compression/lib/get-video-metadata";
import { type ResultError, tryCatch } from "@/lib/try-catch";
import type {
  BuildVariantResult,
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

function getDefaultOptions(profile: LicenseProfile): CompressionOptions {
  const codecsForProfile = getCodecsForProfile(profile);
  const defaultCodec = codecsForProfile[0]?.value ?? "libx264";
  const formatsForProfile = getOutputFormatsForProfile(profile);
  const defaultFormat = formatsForProfile[0]?.value ?? "mp4";
  return resolveOptions(
    {
      quality: 75,
      preset: "fast",
      fps: 30,
      scale: 1,
      removeAudio: false,
      codec: defaultCodec,
      outputFormat: defaultFormat,
      generatePreview: true,
      previewDuration: 3,
      tune: undefined,
    },
    profile
  );
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
  compressionOptions: CompressionOptions;
  /** "full" or "lgpl-macos" from get_build_variant; used to filter codecs/formats. */
  buildVariant: LicenseProfile;
  error: ResultError | null;
  workerState: WorkerState;
  progress: number;
  /** True after FFmpeg event listeners are registered; avoid starting transcode before this. */
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
  setCompressionOptions: (options: CompressionOptions) => void;
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
  compressionOptions: getDefaultOptions("full"),
  buildVariant: "full",
  error: null,
  workerState: WorkerState.Idle,
  progress: 0,
  listenersReady: false,
  ffmpegCommandPreview: null,
  ffmpegCommandPreviewLoading: false,

  initBuildVariant: async () => {
    const result = await tryCatch(
      () => invoke<BuildVariantResult>("get_build_variant"),
      "Build variant"
    );
    if (result.ok) {
      const profile: LicenseProfile =
        result.value.variant === "lgpl-macos" ? "lgpl-macos" : "full";
      set({
        buildVariant: profile,
        compressionOptions: getDefaultOptions(profile),
      });
    }
  },

  refreshFfmpegCommandPreview: async () => {
    const { compressionOptions, inputPath } = get();
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
    void get().refreshFfmpegCommandPreview();

    await tryCatch(
      async () => {
        const { compressionOptions } = get();
        if (compressionOptions.generatePreview) {
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
    if (!inputPath) return;

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
    if (!inputPath) return;

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

    // If requestId was provided (debounced call), check if this request is still current
    if (requestId !== undefined && requestId !== previewRequestId) {
      return; // Stale result, discard
    }

    if (result.ok) {
      set({
        estimatedSize: result.value.estimatedSize,
        videoPreview: {
          originalSrc: convertFileSrc(result.value.originalPath),
          compressedSrc: convertFileSrc(result.value.compressedPath),
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
    const { buildVariant } = get();
    const resolved = resolveOptions(options, buildVariant);
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
