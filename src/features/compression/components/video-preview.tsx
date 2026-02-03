import { useRef } from "react";
import { ReactCompareSlider } from "react-compare-slider";

import { Badge } from "@/components/ui/badge";
import { useVideoSync } from "@/hooks/useVideoSync";
import { cn } from "@/lib/utils";

interface PreviewProps {
  originalSrc: string;
  compressedSrc: string;
  startOffsetSeconds?: number;
  /** Source video FPS. Shown on original when different from previewFps. */
  sourceFps?: number;
  /** Preview/compressed FPS. Shown on compressed when different from sourceFps. */
  previewFps?: number;
}

function getVideoType(src: string): string {
  return src.includes(".webm") ? "video/webm" : "video/mp4";
}

export function VideoPreview({
  originalSrc,
  compressedSrc,
  startOffsetSeconds,
  sourceFps,
  previewFps,
}: PreviewProps) {
  const showFpsBadges =
    sourceFps != null &&
    previewFps != null &&
    sourceFps > 0 &&
    previewFps > 0 &&
    sourceFps !== previewFps;
  const originalVideoRef = useRef<HTMLVideoElement>(null);
  const compressedVideoRef = useRef<HTMLVideoElement>(null);

  useVideoSync(originalVideoRef, compressedVideoRef, startOffsetSeconds ?? 0, [
    originalSrc,
    compressedSrc,
    startOffsetSeconds,
  ]);

  return (
    <div className={cn("relative size-full")}>
      <div className={cn("absolute inset-0")}>
        <ReactCompareSlider
          className={cn("size-full")}
          itemOne={
            <div className="relative size-full">
              <div className="absolute inset-0">
                <video
                  ref={originalVideoRef}
                  muted
                  playsInline
                  preload="none"
                  className={cn("size-full object-contain")}
                >
                  <source src={originalSrc} type="video/mp4" />
                </video>
              </div>
              {showFpsBadges && (
                <Badge className={cn("absolute bottom-2 left-2 z-10")}>
                  {sourceFps} FPS
                </Badge>
              )}
            </div>
          }
          itemTwo={
            <div className="relative size-full">
              <div className="absolute inset-0">
                <video
                  ref={compressedVideoRef}
                  muted
                  playsInline
                  preload="none"
                  className={cn("size-full object-contain")}
                >
                  <source
                    src={compressedSrc}
                    type={getVideoType(compressedSrc)}
                  />
                </video>
              </div>
              {showFpsBadges && (
                <Badge className={cn("absolute right-2 bottom-2 z-10")}>
                  {previewFps} FPS
                </Badge>
              )}
            </div>
          }
        />
      </div>
    </div>
  );
}
