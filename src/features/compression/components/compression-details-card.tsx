import { SquareStop, TrashIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useShallow } from "zustand/react/shallow";

import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { VideoMetadataDisplay } from "@/features/compression/components/video-metadata-display";
import { selectIsActionsDisabled } from "@/features/compression/store/compression-selectors";
import {
  useCompressionStore,
  WorkerState,
} from "@/features/compression/store/compression-store";
import { cn } from "@/lib/utils";

export function CompressionDetailsCard() {
  const {
    inputPath,
    videoMetadata,
    compressionOptions: cOptions,
    estimatedSize,
    workerState,
    isDisabled,
  } = useCompressionStore(
    useShallow((s) => ({
      inputPath: s.inputPath,
      videoMetadata: s.videoMetadata,
      compressionOptions: s.compressionOptions,
      estimatedSize: s.estimatedSize,
      workerState: s.workerState,
      isDisabled: selectIsActionsDisabled(s),
    }))
  );
  const isWorking = workerState !== WorkerState.Idle;
  const isTranscoding = workerState === WorkerState.Transcoding;
  const isGeneratingPreview = workerState === WorkerState.GeneratingPreview;

  const handleCompressOrStop = () => {
    const { terminate, transcodeAndSave } = useCompressionStore.getState();
    if (isWorking) {
      void terminate();
    } else {
      void transcodeAndSave();
    }
  };

  return (
    <AnimatePresence>
      {inputPath && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className={cn("flex flex-col gap-2 rounded-md border bg-card p-4")}
        >
          <div className={cn("flex items-center justify-between")}>
            <h2 className={cn("text-xl font-semibold")}>Details</h2>
            <Button
              size="icon"
              variant="secondary"
              onClick={() => {
                useCompressionStore.getState().clear();
              }}
              className={cn("size-8")}
            >
              <TrashIcon className={cn("size-4")} />
            </Button>
          </div>
          {videoMetadata && cOptions && (
            <VideoMetadataDisplay
              videoMetadata={videoMetadata}
              cOptions={cOptions}
              estimatedSize={estimatedSize}
            />
          )}
          <div
            className={cn("mt-2 flex w-full flex-wrap justify-evenly gap-2")}
          >
            <Button
              className={cn("flex-1")}
              disabled={isDisabled && !isTranscoding}
              onClick={handleCompressOrStop}
            >
              {isWorking && <SquareStop className={cn("size-4")} />}
              {isWorking ? "Stop" : "Compress"}
            </Button>
            {cOptions && !cOptions.generatePreview && (
              <Button
                className={cn("flex-1")}
                variant="secondary"
                onClick={() =>
                  void useCompressionStore.getState().generatePreview()
                }
                disabled={isDisabled || isGeneratingPreview}
              >
                {isGeneratingPreview && <Spinner className={cn("size-4")} />}
                {isGeneratingPreview ? "Processing" : "Generate Preview"}
              </Button>
            )}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
