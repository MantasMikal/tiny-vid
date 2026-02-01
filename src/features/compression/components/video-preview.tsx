import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef } from "react";
import { ReactCompareSlider } from "react-compare-slider";

import { cn } from "@/lib/utils";

interface PreviewProps {
  originalSrc: string;
  compressedSrc: string;
}

export function VideoPreview({ originalSrc, compressedSrc }: PreviewProps) {
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
  }, [originalSrc, compressedSrc]);

  return (
    <ReactCompareSlider
      className={cn("size-full")}
      itemOne={
        <div className="relative size-full">
          <AnimatePresence mode="sync">
            <motion.div
              key={originalSrc}
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.2 }}
              className="absolute inset-0"
            >
              <video
                ref={originalVideoRef}
                muted
                playsInline
                preload="auto"
                className={cn("size-full object-contain")}
                src={originalSrc}
              />
            </motion.div>
          </AnimatePresence>
        </div>
      }
      itemTwo={
        <div className="relative size-full">
          <AnimatePresence mode="sync">
            <motion.div
              key={compressedSrc}
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.2 }}
              className="absolute inset-0"
            >
              <video
                ref={compressedVideoRef}
                muted
                playsInline
                preload="auto"
                className={cn("size-full object-contain")}
                src={compressedSrc}
              />
            </motion.div>
          </AnimatePresence>
        </div>
      }
    />
  );
}
