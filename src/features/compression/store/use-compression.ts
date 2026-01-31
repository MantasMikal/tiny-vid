import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { useShallow } from "zustand/react/shallow";

import { useCompressionStore } from "@/features/compression/store/compression-store";

interface FfmpegErrorPayload {
  summary: string;
  detail: string;
}

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
          const s = useCompressionStore.getState();
          if (s.isTranscoding) {
            useCompressionStore.setState({
              isTranscoding: false,
              error: {
                type: "Transcode Error",
                message: summary,
                detail,
              },
            });
          } else if (s.isGeneratingPreview) {
            useCompressionStore.setState({
              isGeneratingPreview: false,
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
          if (s.isTranscoding) {
            useCompressionStore.setState({
              isTranscoding: false,
              progress: 1,
            });
          } else if (s.isGeneratingPreview) {
            useCompressionStore.setState({ isGeneratingPreview: false });
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
      isTranscoding: s.isTranscoding,
      isGeneratingPreview: s.isGeneratingPreview,
      progress: s.progress,
      videoMetadata: s.videoMetadata,
      estimatedSize: s.estimatedSize,
      compressionOptions: s.compressionOptions,
      listenersReady: s.listenersReady,
      isDisabled:
        !s.listenersReady ||
        s.isTranscoding ||
        s.isGeneratingPreview ||
        s.isSaving,
      isWorking: s.isTranscoding || s.isGeneratingPreview,
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
