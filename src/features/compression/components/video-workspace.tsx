import { AnimatePresence, motion } from "framer-motion";
import { TrashIcon, TriangleAlert } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { Spinner } from "@/components/ui/spinner";
import { VideoPreview } from "@/features/compression/components/video-preview";
import { cn } from "@/lib/utils";

export interface VideoWorkspaceProps {
  inputPath: string | null;
  videoPreview: { originalSrc: string; compressedSrc: string } | null;
  videoUploading: boolean;
  error: { type: string; message: string; detail?: string } | null;
  isGeneratingPreview: boolean;
  isTranscoding: boolean;
  progress: number;
  onBrowse: () => void;
  onClear: () => void;
}

export function VideoWorkspace({
  inputPath,
  videoPreview,
  videoUploading,
  error,
  isGeneratingPreview,
  isTranscoding,
  progress,
  onBrowse,
  onClear,
}: VideoWorkspaceProps) {
  return (
    <div
      className={cn(
        `
          relative flex h-full min-h-[300px] flex-col gap-2 rounded-md border
          bg-card p-2
          md:col-span-2
        `
      )}
    >
      <div className={cn("relative flex h-full items-center justify-center")}>
        {!inputPath ? (
          <div
            className={cn(
              `
                flex size-full cursor-pointer flex-col items-center
                justify-center gap-4 rounded-md border-2 border-dashed
                border-muted-foreground/25 transition-colors
                hover:border-primary/50
              `
            )}
            onClick={() => { onBrowse(); }}
          >
            <p className={cn("text-muted-foreground")}>
              Drop video or click to browse
            </p>
          </div>
        ) : (
          <div
            className={cn(
              `
                relative flex size-full rounded-md bg-black
                md:overflow-hidden
              `
            )}
          >
            {videoUploading && (
              <Spinner
                className={cn("absolute inset-0 z-10 m-auto size-12")}
              />
            )}
            {videoPreview && !videoUploading && (
              <>
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
              </>
            )}
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
            {isGeneratingPreview && videoPreview && (
              <motion.div
                className={cn(
                  `
                    absolute bottom-1 left-1 z-10 flex items-center gap-2
                    rounded-md bg-black/50 p-1 px-2 backdrop-blur-sm
                  `
                )}
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: 10 }}
              >
                <Spinner className={cn("size-4 border-2 border-white")} />
                <span className={cn("text-sm text-white")}>
                  Generating preview
                </span>
              </motion.div>
            )}
            {isTranscoding && (
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
            <Alert variant="destructive" className={cn("bg-black")}>
              <TriangleAlert className={cn("size-5")} />
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
