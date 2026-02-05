import { useRef } from "react";
import { ReactCompareSlider } from "react-compare-slider";
import { useShallow } from "zustand/react/shallow";

import { useCompressionStore } from "@/features/compression/store/compression-store";
import { useVideoSync } from "@/hooks/useVideoSync";
import { cn } from "@/lib/utils";

export function VideoPreview() {
  const originalVideoRef = useRef<HTMLVideoElement>(null);
  const compressedVideoRef = useRef<HTMLVideoElement>(null);

  const { videoPreview, originalSrc, compressedSrc, startOffsetSeconds } = useCompressionStore(
    useShallow((s) => ({
      videoPreview: s.videoPreview,
      originalSrc: s.videoPreview?.originalSrc ?? "",
      compressedSrc: s.videoPreview?.compressedSrc ?? "",
      startOffsetSeconds: s.videoPreview?.startOffsetSeconds,
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
                  <source src={compressedSrc} type="video/mp4" />
                </video>
              </div>
            </div>
          }
        />
      </div>
    </div>
  );
}
