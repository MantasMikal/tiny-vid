import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { useShallow } from "zustand/react/shallow";

import {
  useCompressionStore,
  WorkerState,
} from "@/features/compression/store/compression-store";
import type { FfmpegErrorPayload } from "@/types/tauri";

export function useCompressionStoreInit() {
  useEffect(() => {
    let cancelled = false;
    const unlisteners: (() => void)[] = [];

    const setup = async () => {
      const [unProgress, unError, unComplete] = await Promise.all([
        listen<number>("ffmpeg-progress", (e) => {
          useCompressionStore.setState({ progress: e.payload });
        }),
        listen<FfmpegErrorPayload>("ffmpeg-error", (e) => {
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
        listen("ffmpeg-complete", () => {
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
      ]);
      if (cancelled) {
        unProgress();
        unError();
        unComplete();
        return;
      }
      unlisteners.push(unProgress, unError, unComplete);
      useCompressionStore.setState({ listenersReady: true });
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
    useShallow((s) => ({
      inputPath: s.inputPath,
      videoPreview: s.videoPreview,
      videoUploading: s.videoUploading,
      error: s.error,
      workerState: s.workerState,
      progress: s.progress,
      videoMetadata: s.videoMetadata,
      estimatedSize: s.estimatedSize,
      compressionOptions: s.compressionOptions,
      listenersReady: s.listenersReady,
      isDisabled:
        !s.inputPath ||
        s.isSaving ||
        s.workerState === WorkerState.Transcoding ||
        (!s.listenersReady && s.workerState === WorkerState.Idle),
      isWorking: s.workerState !== WorkerState.Idle,
      selectPath: s.selectPath,
      browseAndSelectFile: s.browseAndSelectFile,
      transcodeAndSave: s.transcodeAndSave,
      clear: s.clear,
      generatePreview: s.generatePreview,
      setCompressionOptions: s.setCompressionOptions,
      terminate: s.terminate,
    }))
  );
}
