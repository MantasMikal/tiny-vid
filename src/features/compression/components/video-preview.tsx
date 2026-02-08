import { Minus, Pause, Play, Plus } from "lucide-react";
import { useEffect, useLayoutEffect, useRef } from "react";
import { ReactCompareSlider } from "react-compare-slider";
import { useShallow } from "zustand/react/shallow";

import { Button } from "@/components/ui/button";
import { PreviewRegionTimeline } from "@/features/compression/components/preview-region-timeline";
import { selectIsInitialized } from "@/features/compression/store/compression-selectors";
import { useCompressionStore } from "@/features/compression/store/compression-store";
import { type VideoSyncRestoreState, useVideoSync } from "@/hooks/useVideoSync";
import { useZoomPan } from "@/hooks/useZoomPan";
import { cn } from "@/lib/utils";

export function VideoPreview() {
  const originalVideoRef = useRef<HTMLVideoElement>(null);
  const compressedVideoRef = useRef<HTMLVideoElement>(null);

  const {
    videoPreview,
    originalSrc,
    compressedSrc,
    startOffsetSeconds,
    videoDuration,
    previewDuration,
    previewStartSeconds,
    isDisabled,
    setPreviewRegionStart,
  } = useCompressionStore(
    useShallow((s) => ({
      videoPreview: s.videoPreview,
      originalSrc: s.videoPreview?.originalSrc ?? "",
      compressedSrc: s.videoPreview?.compressedSrc ?? "",
      startOffsetSeconds: s.videoPreview?.startOffsetSeconds,
      videoDuration: s.videoMetadata?.duration,
      previewDuration: s.compressionOptions?.previewDuration,
      previewStartSeconds: s.previewStartSeconds,
      isDisabled: !selectIsInitialized(s),
      setPreviewRegionStart: s.setPreviewRegionStart,
    }))
  );

  const playbackSnapshotRef = useRef<VideoSyncRestoreState | null>({
    time: 0,
    paused: true,
  });
  const restoreStateRef = useRef<VideoSyncRestoreState | null>(null);
  const lastPreviewStartRef = useRef<number | null>(null);
  const lastPreviewKeyRef = useRef<string | null>(null);

  const isPreviewActive = Boolean(videoPreview);
  const { togglePlayPause, isPaused } = useVideoSync(
    originalVideoRef,
    compressedVideoRef,
    startOffsetSeconds ?? 0,
    [originalSrc, compressedSrc, startOffsetSeconds],
    isPreviewActive,
    restoreStateRef
  );

  const {
    viewportRef,
    transformStyle,
    cursorClassName,
    isPanning,
    zoomPercent,
    canZoomIn,
    canZoomOut,
    handleWheel,
    handlePointerDown,
    handlePointerMove,
    endPan,
    zoomAtCenter,
  } = useZoomPan({
    panExcludeSelector: '[data-rcs="handle-container"]',
  });

  useEffect(() => {
    const video = originalVideoRef.current;
    if (!video) return;
    const updateSnapshot = () => {
      playbackSnapshotRef.current = {
        time: Number.isFinite(video.currentTime) ? video.currentTime : 0,
        paused: video.paused,
      };
    };
    updateSnapshot();
    video.addEventListener("timeupdate", updateSnapshot);
    video.addEventListener("play", updateSnapshot);
    video.addEventListener("pause", updateSnapshot);
    video.addEventListener("seeked", updateSnapshot);
    return () => {
      video.removeEventListener("timeupdate", updateSnapshot);
      video.removeEventListener("play", updateSnapshot);
      video.removeEventListener("pause", updateSnapshot);
      video.removeEventListener("seeked", updateSnapshot);
    };
  }, [originalSrc]);

  useLayoutEffect(() => {
    if (!videoPreview) return;
    const previewKey = `${originalSrc}|${compressedSrc}|${String(startOffsetSeconds ?? 0)}`;
    const lastKey = lastPreviewKeyRef.current;
    if (!lastKey) {
      lastPreviewKeyRef.current = previewKey;
      lastPreviewStartRef.current = previewStartSeconds;
      return;
    }
    if (lastKey === previewKey) return;

    const lastStart = lastPreviewStartRef.current;
    const paused = playbackSnapshotRef.current?.paused ?? true;
    const startTime =
      lastStart !== null && lastStart !== previewStartSeconds
        ? 0
        : playbackSnapshotRef.current?.time ?? 0;
    restoreStateRef.current = {
      time: startTime,
      paused,
    };
    lastPreviewKeyRef.current = previewKey;
    lastPreviewStartRef.current = previewStartSeconds;
  }, [
    videoPreview,
    originalSrc,
    compressedSrc,
    startOffsetSeconds,
    previewStartSeconds,
  ]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.code !== "Space") return;
      const target = event.target;
      if (
        target instanceof HTMLElement &&
        (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable)
      ) {
        return;
      }
      event.preventDefault();
      togglePlayPause();
    };

    window.addEventListener("keydown", handleKeyDown, { passive: false });
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [togglePlayPause]);

  if (!videoPreview) return null;

  return (
    <div className={cn("relative size-full")}>
      <div className={cn("absolute inset-0")}>
        <div
          ref={viewportRef}
          className={cn("absolute inset-0 touch-none overflow-hidden select-none", cursorClassName)}
          onWheel={handleWheel}
          onPointerDownCapture={handlePointerDown}
          onPointerMove={handlePointerMove}
          onPointerUp={endPan}
          onPointerCancel={endPan}
          onPointerLeave={() => {
            endPan();
          }}
        >
          <div className={cn("size-full")} style={transformStyle}>
            <ReactCompareSlider
              className={cn("size-full", isPanning && "pointer-events-none")}
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
      </div>
      <div
        className={cn(
          "pointer-events-auto absolute right-2 bottom-18 z-30",
          "flex items-center gap-1 rounded-xl border bg-background/65 px-2 py-1.5 shadow-lg backdrop-blur-sm"
        )}
      >
        <div className={cn("flex items-center gap-1 rounded-lg bg-foreground/5 p-0.5")}>
          <Button
            variant="ghost"
            size="icon-xs"
            className={cn("size-6")}
            onClick={() => zoomAtCenter("out")}
            aria-label="Zoom out"
            disabled={!canZoomOut}
          >
            <Minus className={cn("size-3")} />
          </Button>
          <span className={cn("min-w-10 text-center text-[10px] font-medium text-foreground/80")}>
            {zoomPercent}%
          </span>
          <Button
            variant="ghost"
            size="icon-xs"
            className={cn("size-6")}
            onClick={() => zoomAtCenter("in")}
            aria-label="Zoom in"
            disabled={!canZoomIn}
          >
            <Plus className={cn("size-3")} />
          </Button>
        </div>
        <div className={cn("mx-1 h-5 w-px bg-foreground/10")} />
        <Button
          variant="ghost"
          size="icon-sm"
          className={cn("size-8")}
          onClick={togglePlayPause}
          aria-label={isPaused ? "Play" : "Pause"}
        >
          {isPaused ? <Play className={cn("size-4")} /> : <Pause className={cn("size-4")} />}
        </Button>
      </div>
      {previewDuration != null && videoDuration != null && (
        <div className={cn("absolute right-0 bottom-0 left-0 z-20 p-2")}>
          <PreviewRegionTimeline
            duration={videoDuration}
            previewDuration={previewDuration}
            startSeconds={previewStartSeconds}
            disabled={isDisabled}
            onStartChange={setPreviewRegionStart}
          />
        </div>
      )}
    </div>
  );
}
