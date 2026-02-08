import type { VideoMetadata } from "@/features/compression/lib/get-video-metadata";
import type { FfmpegSizeEstimate } from "@/types/native";

const BYTES_PER_MB = 1024 * 1024;
const SIZE_UNCHANGED_THRESHOLD = 0.05;

export interface EstimateDisplayState {
  estimatedSizeMB: number;
  estimateLowMB: number;
  estimateHighMB: number;
  confidenceLabel: string;
  deltaLabel: string;
  deltaVariant: "smaller" | "larger" | "unchanged";
  hasTooltipDetails: boolean;
}

/** Returns null when no estimate; otherwise display state for size estimate UI. */
export function computeEstimateDisplayState(
  estimate: FfmpegSizeEstimate | null | undefined,
  originalSizeMB: number
): EstimateDisplayState | null {
  if (estimate == null) return null;

  const estimatedSizeMB = estimate.bestSize / BYTES_PER_MB;
  const estimateLowMB = estimate.lowSize / BYTES_PER_MB;
  const estimateHighMB = estimate.highSize / BYTES_PER_MB;
  const confidenceLabel = `${estimate.confidence.charAt(0).toUpperCase()}${estimate.confidence.slice(1)}`;

  const deltaPercent =
    originalSizeMB !== 0 ? ((estimatedSizeMB - originalSizeMB) / originalSizeMB) * 100 : 0;
  const absoluteDeltaPercent = Math.abs(deltaPercent);
  const isSizeEffectivelyUnchanged = absoluteDeltaPercent < SIZE_UNCHANGED_THRESHOLD;

  const deltaVariant: EstimateDisplayState["deltaVariant"] = isSizeEffectivelyUnchanged
    ? "unchanged"
    : deltaPercent < 0
      ? "smaller"
      : "larger";

  const deltaLabel = isSizeEffectivelyUnchanged
    ? "Same size"
    : `${absoluteDeltaPercent.toFixed(2)}% ${deltaVariant === "smaller" ? "smaller" : "larger"}`;

  return {
    estimatedSizeMB,
    estimateLowMB,
    estimateHighMB,
    confidenceLabel,
    deltaLabel,
    deltaVariant,
    hasTooltipDetails: true,
  };
}

/** True when metadata has any extended detail fields to show in the accordion. */
export function hasExtendedDetails(meta: VideoMetadata): boolean {
  return [
    meta.fps > 0,
    meta.codecName != null,
    meta.codecLongName != null,
    meta.videoBitRate != null,
    meta.formatBitRate != null,
    meta.formatName != null,
    meta.formatLongName != null,
    meta.nbStreams != null,
    meta.audioStreamCount > 0,
    (meta.subtitleStreamCount ?? 0) > 0,
    meta.audioCodecName != null,
    meta.audioChannels != null,
    meta.encoder != null,
  ].some(Boolean);
}
