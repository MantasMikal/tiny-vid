import { useRef } from "react";
import { ReactCompareSlider } from "react-compare-slider";
import { useShallow } from "zustand/react/shallow";

import { Badge } from "@/components/ui/badge";
import { PreviewRegionTimeline } from "@/features/compression/components/preview-region-timeline";
import { selectIsInitialized } from "@/features/compression/store/compression-selectors";
import {
  getCompressionState,
  useCompressionStore,
} from "@/features/compression/store/compression-store";
import { useVideoSync } from "@/hooks/useVideoSync";
import { cn } from "@/lib/utils";

function getVideoType(src: string): string {
  return src.includes(".webm") ? "video/webm" : "video/mp4";
}

export function VideoPreview() {
  const originalVideoRef = useRef<HTMLVideoElement>(null);
  const compressedVideoRef = useRef<HTMLVideoElement>(null);

  const {
    videoPreview,
    originalSrc,
    compressedSrc,
    startOffsetSeconds,
    sourceFps,
    previewFps,
    videoDuration,
    previewDuration,
    previewStartSeconds,
    isDisabled,
  } = useCompressionStore(
    useShallow((s) => ({
      videoPreview: s.videoPreview,
      originalSrc: s.videoPreview?.originalSrc ?? "",
      compressedSrc: s.videoPreview?.compressedSrc ?? "",
      startOffsetSeconds: s.videoPreview?.startOffsetSeconds,
      sourceFps: s.videoMetadata?.fps,
      previewFps: s.compressionOptions?.fps,
      videoDuration: s.videoMetadata?.duration,
      previewDuration: s.compressionOptions?.previewDuration,
      previewStartSeconds: s.previewStartSeconds,
      isDisabled: !selectIsInitialized(s),
    }))
  );

  useVideoSync(originalVideoRef, compressedVideoRef, startOffsetSeconds ?? 0, [
    originalSrc,
    compressedSrc,
    startOffsetSeconds,
  ]);

  if (!videoPreview) return null;

  const showFpsBadges =
    sourceFps != null &&
    previewFps != null &&
    sourceFps > 0 &&
    previewFps > 0 &&
    sourceFps !== previewFps;

  const showPreviewTimeline = previewDuration != null && videoDuration != null;

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
                  preload="none"
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
      {showPreviewTimeline && (
        <div className={cn("absolute bottom-0 z-20 w-full")}>
          <PreviewRegionTimeline
            duration={videoDuration}
            previewDuration={previewDuration}
            startSeconds={previewStartSeconds}
            disabled={isDisabled}
            onStartChange={(startSeconds) => {
              getCompressionState().setPreviewRegionStart(startSeconds);
            }}
          />
        </div>
      )}
    </div>
  );
}
