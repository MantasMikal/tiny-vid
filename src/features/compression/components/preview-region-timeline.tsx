import * as SliderPrimitive from "@radix-ui/react-slider";
import { GripVertical } from "lucide-react";

import { secondsToTimestamp } from "@/features/compression/lib/seconds-to-timestamp";
import { cn } from "@/lib/utils";

interface PreviewRegionTimelineProps {
  duration: number;
  previewDuration: number;
  startSeconds: number;
  disabled?: boolean;
  onStartChange: (startSeconds: number) => void;
}

function formatRangeLabel(startSeconds: number, endSeconds: number, rangeSeconds: number) {
  const range = `${secondsToTimestamp(startSeconds)} – ${secondsToTimestamp(endSeconds)}`;
  const secondsLabel = `${String(Math.max(1, Math.round(rangeSeconds)))}s`;
  return `${range} (${secondsLabel})`;
}

export function PreviewRegionTimeline({
  duration,
  previewDuration,
  startSeconds,
  disabled,
  onStartChange,
}: PreviewRegionTimelineProps) {
  const safeDuration = Number.isFinite(duration) ? Math.max(0, duration) : 0;
  const safePreviewDuration = Number.isFinite(previewDuration) ? Math.max(0, previewDuration) : 0;
  const maxStart = Math.max(0, safeDuration - safePreviewDuration);
  const clampedStart = Math.min(Math.max(0, startSeconds), maxStart);
  const regionWidth = safeDuration > 0 ? Math.min(1, safePreviewDuration / safeDuration) : 0;
  const regionLeft = safeDuration > 0 ? clampedStart / safeDuration : 0;
  const endSeconds = Math.min(safeDuration, clampedStart + safePreviewDuration);
  const rangeSeconds = Math.max(0, endSeconds - clampedStart);
  const isDisabled =
    (disabled ?? false) || safeDuration <= 0 || safeDuration <= safePreviewDuration;
  const regionLeftPercent = `${String(regionLeft * 100)}%`;
  const regionWidthPercent = `${String(regionWidth * 100)}%`;

  return (
    <div
      className={cn(
        `pointer-events-auto rounded-md border bg-background/65 p-2 pb-3 shadow-lg backdrop-blur-sm`,
        isDisabled && "opacity-70"
      )}
    >
      <div className={cn("grid grid-cols-[1fr_auto_1fr] items-center text-[11px]")}>
        <span className={cn("text-left text-foreground/70")}>{secondsToTimestamp(0)}</span>
        <span className={cn("px-2 font-medium text-foreground")}>
          {formatRangeLabel(clampedStart, endSeconds, rangeSeconds)}
        </span>
        <span className={cn("text-right text-foreground/70")}>
          {secondsToTimestamp(safeDuration)}
        </span>
      </div>
      <SliderPrimitive.Root
        className={cn(
          `relative mt-2 flex w-full touch-none items-center select-none data-disabled:opacity-60`
        )}
        min={0}
        max={maxStart}
        step={1}
        value={[clampedStart]}
        disabled={isDisabled}
        onValueChange={(value) => {
          const next = value[0] ?? 0;
          onStartChange(next);
        }}
        aria-label="Preview region start"
      >
        <SliderPrimitive.Track
          className={cn("relative h-2 w-full overflow-hidden rounded-full bg-foreground/15")}
        >
          <div
            className={cn(`absolute inset-y-0 rounded-full bg-primary/50`)}
            style={{
              left: regionLeftPercent,
              width: regionWidthPercent,
            }}
          />
          <SliderPrimitive.Range className={cn("absolute h-full bg-transparent")} />
        </SliderPrimitive.Track>
        <SliderPrimitive.Thumb
          className={cn(
            `flex size-5 cursor-pointer items-center justify-center rounded-full bg-primary hover:bg-primary/80`
          )}
        >
          <GripVertical className={cn("size-3 text-foreground")} />
        </SliderPrimitive.Thumb>
      </SliderPrimitive.Root>
    </div>
  );
}
