import React, { useEffect, useRef } from "react";
import { ReactCompareSlider } from "react-compare-slider";

import { cn } from "@/lib/utils";

interface PreviewProps {
  originalSrc: string;
  compressedSrc: string;
}

function VideoPreviewComponent({ originalSrc, compressedSrc }: PreviewProps) {
  const originalVideoRef = useRef<HTMLVideoElement>(null);
  const compressedVideoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    const originalVideo = originalVideoRef.current;
    const compressedVideo = compressedVideoRef.current;
    let originalEnded = false;
    let compressedEnded = false;

    if (originalVideo && compressedVideo) {
      const handleReady = () => {
        if (
          originalVideo.readyState === 4 &&
          compressedVideo.readyState === 4
        ) {
          void originalVideo.play();
          void compressedVideo.play();
        }
      };

      originalVideo.addEventListener("loadeddata", handleReady);
      compressedVideo.addEventListener("loadeddata", handleReady);

      const tryToPlay = () => {
        if (!originalEnded || !compressedEnded) return;
        void originalVideo.play();
        void compressedVideo.play();
        originalEnded = false;
        compressedEnded = false;
      };

      const handleFirstEnded = () => {
        originalEnded = true;
        tryToPlay();
      };

      const handleSecondEnded = () => {
        compressedEnded = true;
        tryToPlay();
      };

      originalVideo.addEventListener("ended", handleFirstEnded);
      compressedVideo.addEventListener("ended", handleSecondEnded);

      return () => {
        originalVideo.removeEventListener("loadeddata", handleReady);
        compressedVideo.removeEventListener("loadeddata", handleReady);
        originalVideo.removeEventListener("ended", handleFirstEnded);
        compressedVideo.removeEventListener("ended", handleSecondEnded);
      };
    }
  }, []);

  return (
    <ReactCompareSlider
      className={cn("size-full")}
      itemOne={
        <video
          ref={compressedVideoRef}
          muted
          className={cn("size-full object-contain")}
          src={compressedSrc}
        />
      }
      itemTwo={
        <video
          ref={originalVideoRef}
          muted
          className={cn("size-full object-contain")}
          src={originalSrc}
        />
      }
    />
  );
}

export const VideoPreview = React.memo(VideoPreviewComponent);
