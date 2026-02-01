import { SquareStop } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";

import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { VideoMetadataDisplay } from "@/features/compression/components/video-metadata-display";
import type { CompressionOptions } from "@/features/compression/lib/compression-options";
import type { VideoMetadata } from "@/features/compression/lib/get-video-metadata";
import { WorkerState } from "@/features/compression/store/compression-store";
import { cn } from "@/lib/utils";

export interface CompressionDetailsCardProps {
  inputPath: string | null;
  videoMetadata: VideoMetadata | null;
  cOptions: CompressionOptions;
  estimatedSize: number | null;
  isDisabled: boolean;
  workerState: WorkerState;
  onTranscode: () => void;
  onTerminate: () => void;
  onGeneratePreview: () => void;
}

export function CompressionDetailsCard({
  inputPath,
  videoMetadata,
  cOptions,
  estimatedSize,
  isDisabled,
  workerState,
  onTranscode,
  onTerminate,
  onGeneratePreview,
}: CompressionDetailsCardProps) {
  const isWorking = workerState !== WorkerState.Idle;
  const isTranscoding = workerState === WorkerState.Transcoding;
  const isGeneratingPreview = workerState === WorkerState.GeneratingPreview;
  return (
    <AnimatePresence>
      {inputPath && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className={cn("flex flex-col gap-2 rounded-md border bg-card p-4")}
        >
          <h2 className={cn("text-xl font-semibold")}>Details</h2>
          {videoMetadata && (
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
              onClick={() => {
                if (isWorking) {
                  onTerminate();
                } else {
                  onTranscode();
                }
              }}
              disabled={!inputPath || (isDisabled && !isTranscoding)}
            >
              {isWorking && <SquareStop className={cn("size-4")} />}
              {isWorking ? "Stop" : "Compress"}
            </Button>
            {!cOptions.generatePreview && (
              <Button
                className={cn("flex-1")}
                variant="secondary"
                onClick={onGeneratePreview}
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
