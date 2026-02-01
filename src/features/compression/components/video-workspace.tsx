import { TrashIcon, TriangleAlert, XIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { DropZone } from "@/components/ui/drop-zone";
import { Progress } from "@/components/ui/progress";
import { Spinner } from "@/components/ui/spinner";
import { VideoPreview } from "@/features/compression/components/video-preview";
import { WorkerState } from "@/features/compression/store/compression-store";
import { cn } from "@/lib/utils";

export interface VideoWorkspaceProps {
  inputPath: string | null;
  videoPreview: { originalSrc: string; compressedSrc: string } | null;
  videoUploading: boolean;
  error: { type: string; message: string; detail?: string } | null;
  workerState: WorkerState;
  progress: number;
  disabled?: boolean;
  onBrowse: () => void;
  onClear: () => void;
  onDismissError?: () => void;
  onDrop?: (path: string) => void;
}

export function VideoWorkspace({
  inputPath,
  videoPreview,
  videoUploading,
  error,
  workerState,
  progress,
  disabled = false,
  onBrowse,
  onClear,
  onDismissError,
  onDrop,
}: VideoWorkspaceProps) {
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
            disabled={disabled}
            onDrop={(paths) => {
              const path = paths[0];
              if (typeof path === "string") onDrop?.(path);
            }}
            onClick={onBrowse}
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
                  key="video-preview"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.2 }}
                  className="absolute inset-0"
                >
                  <VideoPreview
                    originalSrc={videoPreview.originalSrc}
                    compressedSrc={videoPreview.compressedSrc}
                  />
                  <Button
                    size="icon"
                    variant="secondary"
                    onClick={onClear}
                    className={cn("absolute top-4 right-4 z-10")}
                  >
                    <TrashIcon className={cn("size-5")} />
                  </Button>
                </motion.div>
              )}
            </AnimatePresence>
            {inputPath && !videoPreview && !videoUploading && (
              <div
                className={cn(
                  `
                    flex flex-1 flex-col items-center justify-center
                    text-muted-foreground
                  `
                )}
              >
                <p className={cn("max-w-full truncate px-4")}>
                  {inputPath.split(/[/\\]/).pop()}
                </p>
                <Button
                  size="icon"
                  variant="ghost"
                  onClick={onClear}
                  className={cn("mt-4")}
                >
                  <TrashIcon className={cn("size-5")} />
                </Button>
              </div>
            )}
            {workerState === WorkerState.GeneratingPreview && videoPreview && (
              <motion.div
                className={cn(
                  `
                    absolute bottom-1 left-1 z-10 flex items-center gap-2
                    rounded-md bg-black/70 p-1 px-2
                  `
                )}
                initial={{ opacity: 0, transform: "translateY(10px)" }}
                animate={{ opacity: 1, transform: "translateY(0)" }}
                exit={{ opacity: 0, transform: "translateY(10px)" }}
              >
                <Spinner className={cn("size-4")} />
                <span className={cn("text-sm text-white")}>
                  Generating preview
                </span>
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
            className={cn("absolute bottom-0 left-0 z-10 w-full p-3")}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
          >
            <Alert
              variant="destructive"
              className={cn("relative bg-black pr-10")}
            >
              <TriangleAlert className={cn("size-5")} />
              {onDismissError && (
                <Button
                  size="icon"
                  variant="ghost"
                  onClick={onDismissError}
                  className={cn(
                    `
                      absolute top-2 right-2 size-8 text-current
                      hover:bg-white/20
                    `
                  )}
                >
                  <XIcon className={cn("size-4")} />
                </Button>
              )}
              <AlertTitle>{error.type || "Error"}</AlertTitle>
              <AlertDescription>
                {error.message}
                {error.detail && error.detail !== error.message && (
                  <details className={cn("mt-2")}>
                    <summary
                      className={cn("cursor-pointer text-sm opacity-80")}
                    >
                      Show details
                    </summary>
                    <pre
                      className={cn(
                        `
                          mt-1 max-h-32 overflow-auto text-xs wrap-anywhere
                          whitespace-pre-wrap opacity-90
                        `
                      )}
                    >
                      {error.detail}
                    </pre>
                  </details>
                )}
              </AlertDescription>
            </Alert>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
