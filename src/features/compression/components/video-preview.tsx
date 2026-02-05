import { useRef } from "react";
import { ReactCompareSlider } from "react-compare-slider";
import { useShallow } from "zustand/react/shallow";

import { Badge } from "@/components/ui/badge";
import { useCompressionStore } from "@/features/compression/store/compression-store";
import { useVideoSync } from "@/hooks/useVideoSync";
import { cn } from "@/lib/utils";

function getVideoType(src: string): string {
  return src.includes(".webm") ? "video/webm" : "video/mp4";
}

export function VideoPreview() {
  const originalVideoRef = useRef<HTMLVideoElement>(null);
  const compressedVideoRef = useRef<HTMLVideoElement>(null);

  const { videoPreview, originalSrc, compressedSrc, startOffsetSeconds, sourceFps, previewFps } =
    useCompressionStore(
      useShallow((s) => ({
        videoPreview: s.videoPreview,
        originalSrc: s.videoPreview?.originalSrc ?? "",
        compressedSrc: s.videoPreview?.compressedSrc ?? "",
        startOffsetSeconds: s.videoPreview?.startOffsetSeconds,
        sourceFps: s.videoMetadata?.fps,
        previewFps: s.compressionOptions?.fps,
      }))
    );

  const isPreviewActive = Boolean(videoPreview);

  useVideoSync(
    originalVideoRef,
    compressedVideoRef,
    startOffsetSeconds ?? 0,
    [originalSrc, compressedSrc, startOffsetSeconds],
    isPreviewActive
  );

  if (!videoPreview) return null;

  const showFpsBadges =
    sourceFps != null &&
    previewFps != null &&
    sourceFps > 0 &&
    previewFps > 0 &&
    sourceFps !== previewFps;

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
                  preload="auto"
                  className={cn("size-full object-contain")}
                >
                  <source src={originalSrc} type="video/mp4" />
                </video>
              </div>
              {showFpsBadges && (
                <Badge className={cn("absolute top-2 left-2 z-10")}>{sourceFps} FPS</Badge>
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
                  preload="auto"
                  className={cn("size-full object-contain")}
                >
                  <source src={compressedSrc} type={getVideoType(compressedSrc)} />
                </video>
              </div>
              {showFpsBadges && (
                <Badge className={cn("absolute top-2 right-2 z-10")}>{previewFps} FPS</Badge>
              )}
            </div>
          }
        />
      </div>
    </div>
  );
}
