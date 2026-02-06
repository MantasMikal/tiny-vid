import { AnimatePresence } from "motion/react";
import { useShallow } from "zustand/react/shallow";

import { FadeIn } from "@/components/ui/animations";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Spinner } from "@/components/ui/spinner";
import { CompressionDetailsCard } from "@/features/compression/components/compression-details-card";
import { CompressionErrorAlert } from "@/features/compression/components/compression-error-alert";
import { CompressionProgress } from "@/features/compression/components/compression-progress";
import { InitErrorDisplay } from "@/features/compression/components/init-error-display";
import { PreviewRegionTimeline } from "@/features/compression/components/preview-region-timeline";
import { VideoDropZone } from "@/features/compression/components/video-drop-zone";
import { VideoPreview } from "@/features/compression/components/video-preview";
import { VideoSettings } from "@/features/compression/components/video-settings";
import { getProgressStepLabel } from "@/features/compression/lib/preview-progress";
import { selectIsInitialized } from "@/features/compression/store/compression-selectors";
import {
  getCompressionState,
  useCompressionStore,
  WorkerState,
} from "@/features/compression/store/compression-store";
import { cn } from "@/lib/utils";

export default function Compressor() {
  const {
    initError,
    inputPath,
    videoPreview,
    videoUploading,
    error,
    workerState,
    progress,
    progressStep,
    videoDuration,
    previewDuration,
    previewStartSeconds,
    isDisabled,
    sourceFps,
    previewFps,
  } = useCompressionStore(
    useShallow((s) => ({
      initError: s.initError,
      inputPath: s.inputPath,
      videoPreview: s.videoPreview,
      videoUploading: s.videoUploading,
      error: s.error,
      workerState: s.workerState,
      progress: s.progress,
      progressStep: s.progressStep,
      videoDuration: s.videoMetadata?.duration,
      previewDuration: s.compressionOptions?.previewDuration,
      previewStartSeconds: s.previewStartSeconds,
      isDisabled: !selectIsInitialized(s),
      sourceFps: s.videoMetadata?.fps,
      previewFps: s.compressionOptions?.fps,
    }))
  );

  const progressStepLabel = getProgressStepLabel(progressStep);
  const showProgressOverlay =
    workerState === WorkerState.Transcoding || workerState === WorkerState.GeneratingPreview;
  const showPreviewTimeline =
    videoPreview && !videoUploading && previewDuration != null && videoDuration != null;
  const showFpsBadges =
    videoPreview && (sourceFps ?? 0) > 0 && (previewFps ?? 0) > 0 && sourceFps !== previewFps;

  return (
    <div
      className={cn(
        "mx-auto grid size-full grow items-start gap-4 p-4 pt-2",
        !initError && "md:grid-cols-[1fr_290px] md:overflow-hidden"
      )}
    >
      <div
        className={cn(
          "flex h-full min-h-70 items-center justify-center gap-2 overflow-hidden rounded-md border"
        )}
      >
        {initError && !inputPath && <InitErrorDisplay message={initError} />}
        {!initError && !inputPath && <VideoDropZone />}
        {!initError && inputPath && (
          <div className={cn("relative flex size-full rounded-md bg-background")}>
            {videoPreview && !videoUploading && <VideoPreview />}
            {showPreviewTimeline && (
              <div className={cn("absolute bottom-0 z-20 w-full p-2")}>
                <PreviewRegionTimeline
                  duration={videoDuration}
                  previewDuration={previewDuration}
                  startSeconds={previewStartSeconds}
                  disabled={isDisabled}
                  onStartChange={(startSeconds) => {
                    getCompressionState().setPreviewRegionStart(startSeconds);
                  }}
                />
              </div>
            )}
            <AnimatePresence>
              {videoUploading && (
                <FadeIn key="uploading-spinner">
                  <Spinner className={cn(`absolute inset-2 z-10 m-auto size-12`)} />
                </FadeIn>
              )}
              {showProgressOverlay && (
                <FadeIn
                  key="progress-overlay"
                  delay={0.1}
                  className={cn("absolute top-2 left-2 z-20 w-full max-w-xs")}
                >
                  <CompressionProgress progress={progress} progressStepLabel={progressStepLabel} />
                </FadeIn>
              )}
              {showFpsBadges && (
                <FadeIn key="fps-badges">
                  <Badge className={cn("absolute top-2 left-2 z-10")}>{sourceFps} FPS</Badge>
                  <Badge className={cn("absolute top-2 right-2 z-10")}>{previewFps} FPS</Badge>
                </FadeIn>
              )}
              {error && (
                <FadeIn className={cn("absolute bottom-0 left-0 z-20 w-full p-3")}>
                  <CompressionErrorAlert error={error} />
                </FadeIn>
              )}
            </AnimatePresence>
          </div>
        )}
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
