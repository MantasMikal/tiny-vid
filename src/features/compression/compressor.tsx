import { AnimatePresence } from "motion/react";
import { useShallow } from "zustand/react/shallow";

import { FadeIn } from "@/components/ui/animations";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Spinner } from "@/components/ui/spinner";
import { CompressionDetailsCard } from "@/features/compression/components/compression-details-card";
import { CompressionErrorAlert } from "@/features/compression/components/compression-error-alert";
import { CompressionProgress } from "@/features/compression/components/compression-progress";
import { InitErrorDisplay } from "@/features/compression/components/init-error-display";
import { VideoDropZone } from "@/features/compression/components/video-drop-zone";
import { VideoPreview } from "@/features/compression/components/video-preview";
import { VideoSettings } from "@/features/compression/components/video-settings";
import { getProgressStepLabel } from "@/features/compression/lib/preview-progress";
import { useCompressionStore, WorkerState } from "@/features/compression/store/compression-store";
import { cn } from "@/lib/utils";

export default function Compressor() {
  const initError = useCompressionStore(useShallow((s) => s.initError));
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
        "mx-auto grid size-full grow items-start gap-4 p-4 pt-2",
        !initError && "md:grid-cols-[1fr_290px] md:overflow-hidden"
      )}
    >
      <div
        className={cn(
          "relative flex h-full min-h-[300px] flex-col gap-2 rounded-md border bg-card p-2"
        )}
      >
        <div className={cn("relative flex h-full items-center justify-center")}>
          {initError && !inputPath && <InitErrorDisplay message={initError} />}
          {!initError && !inputPath && <VideoDropZone />}
          {!initError && inputPath && (
            <div
              className={cn("relative flex size-full rounded-md bg-background md:overflow-hidden")}
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
                    <CompressionProgress
                      progress={progress}
                      progressStepLabel={progressStepLabel}
                    />
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
      {!initError && (
        <aside className={cn("flex h-full min-w-0 flex-col gap-4", "md:overflow-hidden")}>
          <div
            className={cn(
              "flex min-w-0 flex-col rounded-md border bg-card p-1",
              "md:overflow-hidden"
            )}
          >
            <ScrollArea className="h-full min-w-0 p-2">
              <div className="flex min-w-0 grow flex-col gap-2 p-1">
                <h2 className={cn("text-xl font-semibold")}>Settings</h2>
                <VideoSettings />
              </div>
            </ScrollArea>
          </div>
          <CompressionDetailsCard />
        </aside>
      )}
    </div>
  );
}
