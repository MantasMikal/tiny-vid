import { AnimatePresence, motion } from "motion/react";
import { useShallow } from "zustand/react/shallow";

import { DropZone } from "@/components/ui/drop-zone";
import { Progress } from "@/components/ui/progress";
import { Spinner } from "@/components/ui/spinner";
import { CompressionErrorAlert } from "@/features/compression/components/compression-error-alert";
import { VideoPreview } from "@/features/compression/components/video-preview";
import { selectIsInitialized } from "@/features/compression/store/compression-selectors";
import {
  useCompressionStore,
  WorkerState,
} from "@/features/compression/store/compression-store";
import { cn } from "@/lib/utils";

export function VideoWorkspace() {
  const {
    inputPath,
    videoPreview,
    videoUploading,
    error,
    workerState,
    progress,
    isDisabled,
  } = useCompressionStore(
    useShallow((s) => ({
      inputPath: s.inputPath,
      videoPreview: s.videoPreview,
      videoUploading: s.videoUploading,
      error: s.error,
      workerState: s.workerState,
      progress: s.progress,
      isDisabled: !selectIsInitialized(s),
    }))
  );
  return (
    <div
      className={cn(
        `
          relative flex h-full min-h-[300px] flex-col gap-2 rounded-md border
          bg-card p-2
        `
      )}
    >
      <div className={cn("relative flex h-full items-center justify-center")}>
        {!inputPath ? (
          <DropZone
            disabled={isDisabled}
            onDrop={(paths) => {
              const path = paths[0];
              if (typeof path === "string")
                void useCompressionStore.getState().selectPath(path);
            }}
            onClick={() =>
              void useCompressionStore.getState().browseAndSelectFile()
            }
          >
            <p className={cn("text-center text-muted-foreground")}>
              Drop video or click to browse
            </p>
          </DropZone>
        ) : (
          <div
            className={cn(
              `
                relative flex size-full rounded-md bg-background
                md:overflow-hidden
              `
            )}
          >
            {videoUploading && (
              <Spinner className={cn("absolute inset-0 z-10 m-auto size-12")} />
            )}
            <AnimatePresence>
              {videoPreview && !videoUploading && (
                <motion.div
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.2 }}
                  className="absolute inset-0"
                >
                  <VideoPreview />
                </motion.div>
              )}
            </AnimatePresence>
            {workerState === WorkerState.GeneratingPreview && videoPreview && (
              <motion.div
                className={cn(
                  `
                    absolute bottom-4 left-1 z-20 flex items-center gap-2
                    rounded-md bg-black/70 p-1 px-2
                  `
                )}
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
              >
                <Spinner className={cn("size-4")} />
              </motion.div>
            )}
            {workerState === WorkerState.Transcoding && (
              <Progress
                className={cn("absolute bottom-0 z-10 w-full")}
                value={progress * 100}
              />
            )}
          </div>
        )}
      </div>
      <AnimatePresence>
        {error && (
          <motion.div
            key={`error-${error.message}`}
            className={cn("absolute bottom-0 left-0 z-20 w-full p-3")}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
          >
            <CompressionErrorAlert error={error} />
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
