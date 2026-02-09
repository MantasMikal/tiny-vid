import { ArrowDown, ArrowUp, Info, Minus } from "lucide-react";

import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import { Badge } from "@/components/ui/badge";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import {
  computeEstimateDisplayState,
  hasExtendedDetails,
} from "@/features/compression/components/estimate-display-state";
import type { VideoMetadata } from "@/features/compression/lib/get-video-metadata";
import { cn } from "@/lib/utils";
import type { FfmpegSizeEstimate } from "@/types/tauri";

interface COptions {
  scale?: number;
}

interface VideoMetadataDisplayProps {
  videoMetadata: VideoMetadata;
  cOptions: COptions;
  estimate?: FfmpegSizeEstimate | null;
  rateControlMode?: "quality" | "targetSize";
  targetSizeMb?: number;
  targetSizeEstimateMb?: number;
}

function formatBitrateMbps(bps: number): string {
  return `${(bps / 1_000_000).toFixed(2)} Mbps`;
}

function formatSizeMB(sizeMB: number): string {
  return `${sizeMB.toFixed(2)} MB`;
}

export function VideoMetadataDisplay({
  videoMetadata,
  cOptions,
  estimate,
  rateControlMode,
  targetSizeMb,
  targetSizeEstimateMb,
}: VideoMetadataDisplayProps) {
  const showEstimate = rateControlMode !== "targetSize";
  const estimateState = showEstimate
    ? computeEstimateDisplayState(estimate, videoMetadata.sizeMB)
    : null;
  const showTargetEstimate = rateControlMode === "targetSize" && targetSizeEstimateMb != null;
  const DeltaIcon =
    estimateState?.deltaVariant === "smaller"
      ? ArrowDown
      : estimateState?.deltaVariant === "larger"
        ? ArrowUp
        : Minus;

  const deltaBadgeClass = cn({
    "border-emerald-500/40 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300":
      estimateState?.deltaVariant === "smaller",
    "border-red-500/40 bg-red-500/10 text-red-700 dark:text-red-300":
      estimateState?.deltaVariant === "larger",
    "border-border bg-muted text-muted-foreground":
      estimateState?.deltaVariant === "unchanged" || estimateState == null,
  });

  return (
    <div className={cn("flex flex-col gap-1")}>
      {/* Primary section - always visible */}
      <div className={cn("mt-0.5 rounded-md border border-border/60 bg-muted/30 px-2.5 py-2")}>
        <div className={cn("flex items-center justify-between gap-2")}>
          <span
            className={cn("text-[11px] font-medium tracking-wide text-muted-foreground uppercase")}
          >
            File size
          </span>
          {estimateState != null && estimateState.hasTooltipDetails && (
            <Tooltip>
              <TooltipTrigger asChild>
                <button
                  type="button"
                  className={cn(
                    `
                      inline-flex size-5 items-center justify-center rounded-full text-muted-foreground
                      hover:text-foreground
                    `
                  )}
                  aria-label="Show estimation details"
                >
                  <Info className={cn("size-3.5")} />
                </button>
              </TooltipTrigger>
              <TooltipContent side="top" sideOffset={8} className={cn("max-w-60")}>
                <div className={cn("flex flex-col gap-1")}>
                  <p>
                    Range: {formatSizeMB(estimateState.estimateLowMB)} -{" "}
                    {formatSizeMB(estimateState.estimateHighMB)}
                  </p>
                  <p>Confidence: {estimateState.confidenceLabel}</p>
                </div>
              </TooltipContent>
            </Tooltip>
          )}
        </div>
        <div className={cn("mt-0.5 text-xs text-muted-foreground")}>
          <span>Resolution: </span>
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

        {showTargetEstimate ? (
          <div className={cn("mt-1 flex flex-wrap items-center gap-2")}>
            <span className={cn("text-muted-foreground line-through")}>
              {formatSizeMB(videoMetadata.sizeMB)}
            </span>
            <span className={cn("font-semibold text-foreground")}>
              ~{formatSizeMB(targetSizeEstimateMb)}
            </span>
            {targetSizeMb != null && (
              <Badge
                variant="outline"
                className={cn("border-border bg-muted text-muted-foreground")}
              >
                Target {formatSizeMB(targetSizeMb)}
              </Badge>
            )}
          </div>
        ) : estimateState != null ? (
          <div className={cn("mt-1 flex flex-wrap items-center gap-2")}>
            <span className={cn("text-muted-foreground line-through")}>
              {formatSizeMB(videoMetadata.sizeMB)}
            </span>
            <span className={cn("font-semibold text-foreground")}>
              ~{formatSizeMB(estimateState.estimatedSizeMB)}
            </span>
            <Badge variant="outline" className={cn("gap-1", deltaBadgeClass)}>
              <DeltaIcon className={cn("size-3")} />
              {estimateState.deltaLabel}
            </Badge>
          </div>
        ) : (
          <div className={cn("mt-1 flex items-center")}>
            <span className={cn("font-semibold text-foreground")}>
              {formatSizeMB(videoMetadata.sizeMB)}
            </span>
          </div>
        )}
      </div>

      {/* Expandable section - all details */}
      {hasExtendedDetails(videoMetadata) && (
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
