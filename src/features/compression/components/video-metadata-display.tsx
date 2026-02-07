import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import type { VideoMetadata } from "@/features/compression/lib/get-video-metadata";
import { secondsToTimestamp } from "@/features/compression/lib/seconds-to-timestamp";
import { cn } from "@/lib/utils";

interface COptions {
  scale?: number;
}

interface VideoMetadataDisplayProps {
  videoMetadata: VideoMetadata;
  cOptions: COptions;
  estimatedSize?: number | null;
}

function formatBitrateMbps(bps: number): string {
  return `${(bps / 1_000_000).toFixed(2)} Mbps`;
}

export function VideoMetadataDisplay({
  videoMetadata,
  cOptions,
  estimatedSize,
}: VideoMetadataDisplayProps) {
  const estimatedSizeMB = estimatedSize != null ? estimatedSize / (1024 * 1024) : null;
  const percent =
    estimatedSizeMB != null && videoMetadata.sizeMB !== 0
      ? (((videoMetadata.sizeMB - estimatedSizeMB) / videoMetadata.sizeMB) * 100).toFixed(2)
      : null;

  const hasExtendedDetails = [
    videoMetadata.fps > 0,
    videoMetadata.codecName != null,
    videoMetadata.codecLongName != null,
    videoMetadata.videoBitRate != null,
    videoMetadata.formatBitRate != null,
    videoMetadata.formatName != null,
    videoMetadata.formatLongName != null,
    videoMetadata.nbStreams != null,
    videoMetadata.audioStreamCount > 0,
    (videoMetadata.subtitleStreamCount ?? 0) > 0,
    videoMetadata.audioCodecName != null,
    videoMetadata.audioChannels != null,
    videoMetadata.encoder != null,
  ].some(Boolean);

  return (
    <div className={cn("flex flex-col gap-1")}>
      {/* Primary section - always visible */}
      <p className={cn("text-sm text-foreground")}>
        <b>Duration:</b> {secondsToTimestamp(videoMetadata.duration)}
      </p>
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
      <div className={cn("text-sm text-foreground")}>
        <b>File size:</b>{" "}
        {estimatedSizeMB != null ? (
          <>
            <span className={cn("line-through")}>{videoMetadata.sizeMB.toFixed(2)} MB</span>{" "}
            <span>{estimatedSizeMB.toFixed(2)} MB</span>{" "}
            {percent != null && <span>({percent}% reduction)</span>}
          </>
        ) : (
          `${videoMetadata.sizeMB.toFixed(2)} MB`
        )}
      </div>

      {/* Expandable section - all details */}
      {hasExtendedDetails && (
        <Accordion type="single" collapsible className={cn("w-full")}>
          <AccordionItem value="all-details" className={cn("border-none")}>
            <AccordionTrigger className={cn("py-2", "hover:no-underline")}>
              Show all details
            </AccordionTrigger>
            <AccordionContent className={cn("pt-0 pb-2")}>
              <div className={cn("flex flex-col gap-1 text-sm text-foreground")}>
                {videoMetadata.fps > 0 && (
                  <p>
                    <b>Frame rate:</b> {videoMetadata.fps} fps
                  </p>
                )}
                {videoMetadata.codecName != null && (
                  <p>
                    <b>Codec:</b> {videoMetadata.codecName}
                    {videoMetadata.codecLongName != null &&
                      videoMetadata.codecLongName !== videoMetadata.codecName && (
                        <span className={cn("text-muted-foreground")}>
                          {" "}
                          ({videoMetadata.codecLongName})
                        </span>
                      )}
                  </p>
                )}
                {videoMetadata.encoder != null && (
                  <p>
                    <b>Encoder:</b> {videoMetadata.encoder}
                  </p>
                )}
                {videoMetadata.videoBitRate != null && (
                  <p>
                    <b>Video bitrate:</b> {formatBitrateMbps(videoMetadata.videoBitRate)}
                  </p>
                )}
                {videoMetadata.formatBitRate != null && (
                  <p>
                    <b>Format bitrate:</b> {formatBitrateMbps(videoMetadata.formatBitRate)}
                  </p>
                )}
                {videoMetadata.formatName != null && (
                  <p>
                    <b>Container:</b> {videoMetadata.formatLongName ?? videoMetadata.formatName}
                  </p>
                )}
                {videoMetadata.nbStreams != null && (
                  <p>
                    <b>Streams:</b> {videoMetadata.nbStreams}
                  </p>
                )}
                {videoMetadata.audioStreamCount > 0 && (
                  <p>
                    <b>Audio streams:</b> {videoMetadata.audioStreamCount}
                  </p>
                )}
                {(videoMetadata.subtitleStreamCount ?? 0) > 0 && (
                  <p>
                    <b>Subtitle streams:</b> {videoMetadata.subtitleStreamCount}
                  </p>
                )}
                {videoMetadata.audioCodecName != null && (
                  <p>
                    <b>Audio codec:</b> {videoMetadata.audioCodecName}
                    {videoMetadata.audioChannels != null && (
                      <span className={cn("text-muted-foreground")}>
                        {" "}
                        ({videoMetadata.audioChannels} ch)
                      </span>
                    )}
                  </p>
                )}
                {videoMetadata.audioCodecName == null && videoMetadata.audioChannels != null && (
                  <p>
                    <b>Audio channels:</b> {videoMetadata.audioChannels}
                  </p>
                )}
              </div>
            </AccordionContent>
          </AccordionItem>
        </Accordion>
      )}
    </div>
  );
}
