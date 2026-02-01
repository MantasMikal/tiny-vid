import { ScrollArea } from "@/components/ui/scroll-area";
import { CompressionDetailsCard } from "@/features/compression/components/compression-details-card";
import { VideoSettings } from "@/features/compression/components/video-settings";
import { VideoWorkspace } from "@/features/compression/components/video-workspace";
import { useCompressionStore } from "@/features/compression/store/compression-store";
import { useCompression } from "@/features/compression/store/use-compression";
import { cn } from "@/lib/utils";

export default function Compressor() {
  const {
    inputPath,
    videoPreview,
    videoUploading,
    error,
    workerState,
    progress,
    videoMetadata,
    estimatedSize,
    compressionOptions,
    isDisabled,
    browseAndSelectFile,
    clear,
    setCompressionOptions,
    transcodeAndSave,
    terminate,
    generatePreview,
  } = useCompression();

  return (
    <div
      className={cn(
        `
          mx-auto grid size-full grow items-start gap-4 p-4 pt-2
          md:grid-cols-3 md:overflow-hidden
        `
      )}
    >
      <VideoWorkspace
        inputPath={inputPath}
        videoPreview={videoPreview}
        videoUploading={videoUploading}
        error={error}
        workerState={workerState}
        progress={progress}
        onBrowse={() => void browseAndSelectFile()}
        onClear={clear}
        onDrop={(path: string) =>
          void useCompressionStore.getState().selectPath(path)
        }
      />
      <aside
        className={cn(
          `
            col-span-1 flex h-full min-w-0 flex-col gap-4
            md:overflow-hidden
          `
        )}
      >
        <div
          className={cn(
            `
              flex min-w-0 flex-col rounded-md border bg-card p-1
              md:overflow-hidden
            `
          )}
        >
          <ScrollArea className={cn("h-full min-w-0 p-2")}>
            <div className={cn("flex min-w-0 grow flex-col gap-2 p-1")}>
              <h2 className={cn("text-xl font-semibold")}>Settings</h2>
              <VideoSettings
                isDisabled={isDisabled}
                cOptions={compressionOptions}
                onOptionsChange={setCompressionOptions}
              />
            </div>
          </ScrollArea>
        </div>
        <CompressionDetailsCard
          inputPath={inputPath}
          videoMetadata={videoMetadata}
          cOptions={compressionOptions}
          estimatedSize={estimatedSize}
          isDisabled={isDisabled}
          workerState={workerState}
          onTranscode={() => void transcodeAndSave()}
          onTerminate={() => void terminate()}
          onGeneratePreview={() => void generatePreview()}
        />
      </aside>
    </div>
  );
}
