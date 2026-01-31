import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { create } from "zustand";

import type { CompressionOptions } from "@/features/compression/lib/compression-options";
import type { VideoMetadata } from "@/features/compression/lib/get-video-metadata";
import { getVideoMetadataFromPath } from "@/features/compression/lib/get-video-metadata";
import { type ResultError, tryCatch } from "@/lib/try-catch";

export interface VideoPreview {
  originalSrc: string;
  compressedSrc: string;
}

function toRustOptions(opts: CompressionOptions) {
  return {
    codec: opts.codec,
    quality: opts.quality,
    maxBitrate: opts.maxBitrate,
    scale: opts.scale,
    fps: opts.fps,
    removeAudio: opts.removeAudio,
    preset: opts.preset,
    tune: opts.tune,
    previewDuration: opts.previewDuration ?? 3,
  };
}

const DEFAULT_COMPRESSION_OPTIONS: CompressionOptions = {
  quality: 75,
  preset: "fast",
  fps: 30,
  scale: 1,
  removeAudio: false,
  codec: "libx264",
  generatePreview: true,
  previewDuration: 3,
  tune: undefined,
};

let debouncePreviewTimer: ReturnType<typeof setTimeout> | null = null;

interface CompressionState {
  inputPath: string | null;
  videoPreview: VideoPreview | null;
  videoUploading: boolean;
  estimatedSize: number | null;
  videoMetadata: VideoMetadata | null;
  isSaving: boolean;
  compressionOptions: CompressionOptions;
  error: ResultError | null;
  isTranscoding: boolean;
  isGeneratingPreview: boolean;
  progress: number;
  /** True after FFmpeg event listeners are registered; avoid starting transcode before this. */
  listenersReady: boolean;

  selectPath: (path: string) => Promise<void>;
  browseAndSelectFile: () => Promise<void>;
  transcodeAndSave: () => Promise<void>;
  clear: () => void;
  generatePreview: () => Promise<void>;
  setCompressionOptions: (options: CompressionOptions) => void;
  terminate: () => Promise<void>;
}

export const useCompressionStore = create<CompressionState>((set, get) => ({
  inputPath: null,
  videoPreview: null,
  videoUploading: false,
  estimatedSize: null,
  videoMetadata: null,
  isSaving: false,
  compressionOptions: DEFAULT_COMPRESSION_OPTIONS,
  error: null,
  isTranscoding: false,
  isGeneratingPreview: false,
  progress: 0,
  listenersReady: false,

  selectPath: async (path: string) => {
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

    await tryCatch(
      async () => {
        const { compressionOptions } = get();
        if (compressionOptions.generatePreview) {
          set({ isGeneratingPreview: true, error: null });
          const result = await tryCatch(
            () =>
              invoke<{
                original_path: string;
                compressed_path: string;
                estimated_size: number;
              }>("ffmpeg_preview", {
                inputPath: path,
                options: toRustOptions(compressionOptions),
              }),
            "Preview Error"
          );
          if (result.ok) {
            set({
              estimatedSize: result.value.estimated_size,
              videoPreview: {
                originalSrc: convertFileSrc(result.value.original_path),
                compressedSrc: convertFileSrc(result.value.compressed_path),
              },
              isGeneratingPreview: false,
            });
          } else if (!result.aborted) {
            set({
              isGeneratingPreview: false,
              error: result.error,
            });
            void get().terminate();
          }
        }
      },
      "Preview Error",
      { onFinally: () => { set({ videoUploading: false }); } }
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
    const { inputPath, compressionOptions } = get();
    if (!inputPath) return;

    set({ isTranscoding: true, progress: 0, error: null });
    const transcodeResult = await tryCatch(
      () =>
        invoke<string>("ffmpeg_transcode_to_temp", {
          inputPath,
          options: toRustOptions(compressionOptions),
        }),
      "Transcode Error"
    );
    if (!transcodeResult.ok) {
      if (!transcodeResult.aborted) {
        set({
          isTranscoding: false,
          error: transcodeResult.error,
        });
      }
      await get().terminate();
      return;
    }
    const tempPath = transcodeResult.value;
    set({ isTranscoding: false, progress: 1 });

    set({ isSaving: true });
    await tryCatch(
      async () => {
        const outputPath = await save({
          defaultPath: `compressed-${inputPath.split(/[/\\]/).pop() ?? "output.mp4"}`,
          filters: [{ name: "Video", extensions: ["mp4"] }],
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
      { onFinally: () => { set({ isSaving: false }); } }
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
  },

  generatePreview: async () => {
    const { inputPath, compressionOptions } = get();
    if (!inputPath) return;

    set({ isGeneratingPreview: true, error: null });
    const result = await tryCatch(
      () =>
        invoke<{
          original_path: string;
          compressed_path: string;
          estimated_size: number;
        }>("ffmpeg_preview", {
          inputPath,
          options: toRustOptions(compressionOptions),
        }),
      "Preview Error"
    );
    if (result.ok) {
      set({
        estimatedSize: result.value.estimated_size,
        videoPreview: {
          originalSrc: convertFileSrc(result.value.original_path),
          compressedSrc: convertFileSrc(result.value.compressed_path),
        },
        isGeneratingPreview: false,
      });
    } else if (!result.aborted) {
      set({
        isGeneratingPreview: false,
        error: result.error,
      });
      await get().terminate();
    }
  },

  setCompressionOptions: (
    options: CompressionOptions,
    opts?: { triggerPreview?: boolean }
  ) => {
    set({ compressionOptions: options });
    if (opts?.triggerPreview === false) return;
    if (!options.generatePreview) return;

    const { inputPath } = get();
    if (!inputPath) return;

    if (debouncePreviewTimer) {
      clearTimeout(debouncePreviewTimer);
    }

    debouncePreviewTimer = setTimeout(() => {
      debouncePreviewTimer = null;
      void get().generatePreview();
    }, 300);
  },

  terminate: async () => {
    await tryCatch(
      () => invoke("ffmpeg_terminate"),
      "Terminate Error"
    );
    set({
      isTranscoding: false,
      isGeneratingPreview: false,
      progress: 0,
    });
  },
}));
