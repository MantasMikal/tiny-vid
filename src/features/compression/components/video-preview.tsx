import { useRef } from "react";
import { ReactCompareSlider } from "react-compare-slider";

import { useVideoSync } from "@/hooks/useVideoSync";
import { cn } from "@/lib/utils";

interface PreviewProps {
  originalSrc: string;
  compressedSrc: string;
  startOffsetSeconds?: number;
}

function getVideoType(src: string): string {
  return src.includes(".webm") ? "video/webm" : "video/mp4";
}

export function VideoPreview({
  originalSrc,
  compressedSrc,
  startOffsetSeconds,
}: PreviewProps) {
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
            </div>
          }
        />
      </div>
    </div>
  );
}
