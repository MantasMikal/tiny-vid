import { secondsToTimestamp } from "@/features/compression/lib/seconds-to-timestamp";
import { cn } from "@/lib/utils";

interface VideoMetadata {
  duration?: number;
  width?: number;
  height?: number;
  sizeMB?: number;
}

interface COptions {
  scale?: number;
}

interface VideoMetadataDisplayProps {
  videoMetadata: VideoMetadata;
  cOptions: COptions;
  estimatedSize?: number | null;
}

export function VideoMetadataDisplay({
  videoMetadata,
  cOptions,
  estimatedSize,
}: VideoMetadataDisplayProps) {
  const estimatedSizeMB =
    estimatedSize != null ? estimatedSize / (1024 * 1024) : null;
  const percent =
    videoMetadata.sizeMB && estimatedSizeMB && videoMetadata.sizeMB !== 0
      ? (
          ((videoMetadata.sizeMB - estimatedSizeMB) / videoMetadata.sizeMB) *
          100
        ).toFixed(2)
      : null;

  return (
    <div className={cn("flex flex-col gap-1")}>
      {videoMetadata.duration != null && (
        <p className={cn("text-sm text-foreground")}>
          <b>Video Duration:</b> {secondsToTimestamp(videoMetadata.duration)}
        </p>
      )}
      {videoMetadata.width != null && videoMetadata.height != null && (
        <div className={cn("text-sm text-foreground")}>
          <b>Resolution:</b>{" "}
          {cOptions.scale != null && cOptions.scale !== 1 ? (
            <>
              <span className={cn("line-through")}>
                {String(videoMetadata.width)}x{String(videoMetadata.height)}
              </span>{" "}
              <span>
                {(videoMetadata.width * cOptions.scale).toFixed(0)}x
                {(videoMetadata.height * cOptions.scale).toFixed(0)}
              </span>
            </>
          ) : (
            `${String(videoMetadata.width)}x${String(videoMetadata.height)}`
          )}
        </div>
      )}
      {videoMetadata.sizeMB != null && (
        <div className={cn("text-sm text-foreground")}>
          <b>File size: </b>
          {estimatedSizeMB != null ? (
            <>
              <span className={cn("line-through")}>
                {videoMetadata.sizeMB.toFixed(2)}MB
              </span>{" "}
              <span>{estimatedSizeMB.toFixed(2)}MB</span>{" "}
              {percent != null && <span>{percent}%</span>}
            </>
          ) : (
            `${videoMetadata.sizeMB.toFixed(2)}MB`
          )}
        </div>
      )}
    </div>
  );
}
