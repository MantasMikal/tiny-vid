import { SquareStop, TrashIcon } from "lucide-react";
import { AnimatePresence } from "motion/react";
import { useShallow } from "zustand/react/shallow";

import { FadeIn } from "@/components/ui/animations";
import { Button } from "@/components/ui/button";
import { VideoMetadataDisplay } from "@/features/compression/components/video-metadata-display";
import { selectIsActionsDisabled } from "@/features/compression/store/compression-selectors";
import { useCompressionStore, WorkerState } from "@/features/compression/store/compression-store";
import { cn } from "@/lib/utils";

export function CompressionDetailsCard() {
  const {
    inputPath,
    videoMetadata,
    compressionOptions: cOptions,
    estimate,
    workerState,
    isDisabled,
  } = useCompressionStore(
    useShallow((s) => ({
      inputPath: s.inputPath,
      videoMetadata: s.videoMetadata,
      compressionOptions: s.compressionOptions,
      estimate: s.estimate,
      workerState: s.workerState,
      isDisabled: selectIsActionsDisabled(s),
    }))
  );
  const isTranscoding = workerState === WorkerState.Transcoding;
  const isGeneratingPreview = workerState === WorkerState.GeneratingPreview;

  const handleCompressOrStop = () => {
    const { terminate, transcodeAndSave } = useCompressionStore.getState();
    if (isTranscoding) {
      void terminate();
    } else {
      void transcodeAndSave();
    }
  };

  const handleGeneratePreviewOrStop = () => {
    const { terminate, generatePreview } = useCompressionStore.getState();
    if (isGeneratingPreview) {
      void terminate();
    } else {
      void generatePreview();
    }
  };

  const isManualPreview = cOptions && !cOptions.generatePreview;

  return (
    <AnimatePresence>
      {inputPath && (
        <FadeIn className={cn("flex flex-col gap-2 rounded-md border bg-card p-4")}>
          <div className={cn("flex items-center justify-between")}>
            <h2 className={cn("text-xl font-semibold")}>Details</h2>
            <Button
              size="icon"
              variant="destructive"
              onClick={() => {
                const { terminate, clear } = useCompressionStore.getState();
                void terminate().finally(() => {
                  clear();
                });
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
              estimate={estimate}
            />
          )}
          <div className={cn("mt-2 flex w-full gap-2")}>
            <Button
              className={cn("grow")}
              disabled={isGeneratingPreview || (isDisabled && !isTranscoding)}
              onClick={handleCompressOrStop}
            >
              {isTranscoding && <SquareStop className={cn("size-4")} />}
              {isTranscoding ? "Stop" : "Compress"}
            </Button>
            {isManualPreview && (
              <Button
                className={cn("min-w-[100px]")}
                variant="secondary"
                onClick={handleGeneratePreviewOrStop}
                disabled={isDisabled}
              >
                {isGeneratingPreview && <SquareStop className={cn("size-4")} />}
                {isGeneratingPreview ? "Stop" : "Preview"}
              </Button>
            )}
          </div>
        </FadeIn>
      )}
    </AnimatePresence>
  );
}
