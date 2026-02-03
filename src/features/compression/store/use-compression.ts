import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef } from "react";
import { useShallow } from "zustand/react/shallow";

import {
  useCompressionStore,
  WorkerState,
} from "@/features/compression/store/compression-store";
import type { FfmpegErrorPayload } from "@/types/tauri";

export function useCompressionStoreInit() {
  const effectIdRef = useRef(0);

  useEffect(() => {
    const effectId = ++effectIdRef.current;
    let cancelled = false;
    const unlisteners: (() => void)[] = [];
    const win = getCurrentWindow();

    const setup = async () => {
      await useCompressionStore.getState().initBuildVariant();
      const [unProgress, unError, unComplete, unOpenFile, unMenuOpenFile] =
        await Promise.all([
          win.listen<number>("ffmpeg-progress", (e) => {
            useCompressionStore.setState({ progress: e.payload });
          }),
          win.listen<FfmpegErrorPayload>("ffmpeg-error", (e) => {
            const { summary, detail } = e.payload;
            if (summary === "Aborted") return;
            const s = useCompressionStore.getState();
            if (s.workerState === WorkerState.Transcoding) {
              useCompressionStore.setState({
                workerState: WorkerState.Idle,
                error: {
                  type: "Transcode Error",
                  message: summary,
                  detail,
                },
              });
            } else if (s.workerState === WorkerState.GeneratingPreview) {
              useCompressionStore.setState({
                workerState: WorkerState.Idle,
                error: {
                  type: "Preview Error",
                  message: summary,
                  detail,
                },
              });
            }
          }),
          win.listen("ffmpeg-complete", () => {
            const s = useCompressionStore.getState();
            if (s.workerState === WorkerState.Transcoding) {
              useCompressionStore.setState({
                workerState: WorkerState.Idle,
                progress: 1,
              });
            } else if (s.workerState === WorkerState.GeneratingPreview) {
              useCompressionStore.setState({ workerState: WorkerState.Idle });
            }
          }),
          win.listen<string[]>("open-file", (e) => {
            const paths = e.payload;
            if (Array.isArray(paths) && paths.length > 0) {
              void useCompressionStore.getState().selectPath(paths[0]);
            }
          }),
          win.listen("menu-open-file", () => {
            void useCompressionStore.getState().browseAndSelectFile();
          }),
        ]);
      if (cancelled || effectId !== effectIdRef.current) {
        unProgress();
        unError();
        unComplete();
        unOpenFile();
        unMenuOpenFile();
        return;
      }
      unlisteners.push(
        unProgress,
        unError,
        unComplete,
        unOpenFile,
        unMenuOpenFile
      );
      useCompressionStore.setState({ listenersReady: true });

      const pendingPaths = await invoke<string[]>("get_pending_opened_files");
      if (pendingPaths.length > 0) {
        void useCompressionStore.getState().selectPath(pendingPaths[0]);
      }
    };

    void setup();

    return () => {
      cancelled = true;
      useCompressionStore.setState({ listenersReady: false });
      unlisteners.forEach((u) => {
        u();
      });
    };
  }, []);
}

export function useCompression() {
  return useCompressionStore(
    useShallow((s) => {
      const isInitialized = s.availableCodecs.length > 0 && s.listenersReady;
      return {
        inputPath: s.inputPath,
        videoPreview: s.videoPreview,
        videoUploading: s.videoUploading,
        error: s.error,
        workerState: s.workerState,
        progress: s.progress,
        videoMetadata: s.videoMetadata,
        estimatedSize: s.estimatedSize,
        compressionOptions: s.compressionOptions,
        availableCodecs: s.availableCodecs,
        initError: s.initError,
        listenersReady: s.listenersReady,
        ffmpegCommandPreview: s.ffmpegCommandPreview,
        ffmpegCommandPreviewLoading: s.ffmpegCommandPreviewLoading,
        isInitialized,
        isDisabled:
          !isInitialized ||
          s.compressionOptions == null ||
          !s.inputPath ||
          s.isSaving ||
          s.workerState === WorkerState.Transcoding,
        isWorking: s.workerState !== WorkerState.Idle,
        selectPath: s.selectPath,
        browseAndSelectFile: s.browseAndSelectFile,
        transcodeAndSave: s.transcodeAndSave,
        clear: s.clear,
        dismissError: s.dismissError,
        generatePreview: s.generatePreview,
        setCompressionOptions: s.setCompressionOptions,
        refreshFfmpegCommandPreview: s.refreshFfmpegCommandPreview,
        terminate: s.terminate,
      };
    })
  );
}
