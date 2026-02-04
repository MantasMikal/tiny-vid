import { AnimatePresence } from "motion/react";
import { useShallow } from "zustand/react/shallow";

import { FadeIn } from "@/components/ui/animations";
import { Spinner } from "@/components/ui/spinner";
import { CompressionErrorAlert } from "@/features/compression/components/compression-error-alert";
import { CompressionProgress } from "@/features/compression/components/compression-progress";
import { VideoDropZone } from "@/features/compression/components/video-drop-zone";
import { VideoPreview } from "@/features/compression/components/video-preview";
import { getProgressStepLabel } from "@/features/compression/lib/preview-progress";
import { useCompressionStore, WorkerState } from "@/features/compression/store/compression-store";
import { cn } from "@/lib/utils";

export function VideoWorkspace() {
  const { inputPath, videoPreview, videoUploading, error, workerState, progress, progressStep } =
    useCompressionStore(
      useShallow((s) => ({
        inputPath: s.inputPath,
        videoPreview: s.videoPreview,
        videoUploading: s.videoUploading,
        error: s.error,
        workerState: s.workerState,
        progress: s.progress,
        progressStep: s.progressStep,
      }))
    );

  const progressStepLabel = getProgressStepLabel(progressStep);

  const showProgressOverlay =
    workerState === WorkerState.Transcoding || workerState === WorkerState.GeneratingPreview;
  return (
    <div
      className={cn(
        `relative flex h-full min-h-[300px] flex-col gap-2 rounded-md border bg-card p-2`
      )}
    >
      <div className={cn("relative flex h-full items-center justify-center")}>
        {!inputPath && <VideoDropZone />}
        {inputPath && (
          <div
            className={cn(`relative flex size-full rounded-md bg-background md:overflow-hidden`)}
          >
            {videoUploading && <Spinner className={cn("absolute inset-0 z-10 m-auto size-12")} />}
            <AnimatePresence>
              {videoPreview && !videoUploading && (
                <FadeIn className="absolute inset-0">
                  <VideoPreview />
                </FadeIn>
              )}
            </AnimatePresence>
            <AnimatePresence>
              {showProgressOverlay && (
                <FadeIn className={cn("absolute top-0 left-0 z-20 w-full max-w-xs")}>
                  <CompressionProgress progress={progress} progressStepLabel={progressStepLabel} />
                </FadeIn>
              )}
            </AnimatePresence>
          </div>
        )}
      </div>
      <AnimatePresence>
        {error && (
          <FadeIn className={cn("absolute bottom-0 left-0 z-20 w-full p-3")}>
            <CompressionErrorAlert error={error} />
          </FadeIn>
        )}
      </AnimatePresence>
    </div>
  );
}
