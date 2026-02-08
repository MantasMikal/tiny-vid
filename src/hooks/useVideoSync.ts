import { type DependencyList, type RefObject, useCallback, useEffect, useState } from "react";

const DEBUG = import.meta.env.DEV && import.meta.env.VITE_VIDEO_SYNC_DEBUG === "1";

const SYNC_INTERVAL_MS = 100;
const SEEK_THRESHOLD_SECONDS = 0.08;

function clampTime(time: number, video: HTMLVideoElement) {
  const duration = video.duration;
  if (!Number.isFinite(duration) || duration <= 0) {
    return Math.max(0, time);
  }
  return Math.min(Math.max(time, 0), Math.max(0, duration - 0.001));
}

function safePlay(video: HTMLVideoElement) {
  void video.play().catch(() => undefined);
}

function safePause(video: HTMLVideoElement) {
  if (!video.paused) {
    video.pause();
  }
}

function round3(n: number) {
  return Math.round(n * 1000) / 1000;
}

export function useVideoSync(
  primaryRef: RefObject<HTMLVideoElement | null>,
  secondaryRef: RefObject<HTMLVideoElement | null>,
  startOffsetSeconds = 0,
  deps: DependencyList = [],
  enabled = true
): { togglePlayPause: () => void; isPaused: boolean } {
  const [isPaused, setIsPaused] = useState(true);

  const togglePlayPause = useCallback(() => {
    const primary = primaryRef.current;
    if (!primary) return;
    if (primary.paused) {
      void primary.play().catch(() => undefined);
    } else {
      primary.pause();
    }
  }, [primaryRef]);

  useEffect(() => {
    if (!enabled) return;

    let retryRafId: number | null = null;
    let innerCleanup: (() => void) | undefined;

    const run = () => {
      const primary = primaryRef.current;
      const secondary = secondaryRef.current;

      if (!primary || !secondary) {
        retryRafId = requestAnimationFrame(run);
        return;
      }

      const initAt = performance.now();
      let intervalId: ReturnType<typeof setInterval> | null = null;
      let hasStarted = false;
      let pendingSecondaryResume = false;
      const offset = Number.isFinite(startOffsetSeconds) ? Math.max(0, startOffsetSeconds) : 0;

      const log = (event: string, data?: unknown) => {
        if (!DEBUG) return;
        const sinceInitMs = Math.round(performance.now() - initAt);
        console.debug(`[video-sync] +${String(sinceInitMs)}ms ${event}`, data);
      };

      const stopLoop = () => {
        if (intervalId !== null) {
          clearInterval(intervalId);
          intervalId = null;
        }
      };

      const startLoop = () => {
        if (intervalId !== null) return;
        intervalId = setInterval(() => {
          sync("interval");
        }, SYNC_INTERVAL_MS);
      };

      const getPrimaryLoopStart = () => {
        if (offset <= 0) return 0;
        const duration = primary.duration;
        if (Number.isFinite(duration) && duration > 0 && offset >= duration) {
          return 0;
        }
        return clampTime(offset, primary);
      };

      const resetForLoop = () => {
        if (primary.readyState >= HTMLMediaElement.HAVE_METADATA) {
          const loopStart = getPrimaryLoopStart();
          if (Math.abs(primary.currentTime - loopStart) > 0.001) {
            primary.currentTime = loopStart;
          }
        }
        if (
          secondary.readyState >= HTMLMediaElement.HAVE_METADATA &&
          Math.abs(secondary.currentTime) > 0.001
        ) {
          secondary.currentTime = 0;
        }
        pendingSecondaryResume = false;
        safePause(secondary);
      };

      const sync = (reason: string) => {
        if (primary.readyState < HTMLMediaElement.HAVE_METADATA) return;
        if (secondary.readyState < HTMLMediaElement.HAVE_METADATA) return;

        if (primary.paused || primary.seeking || primary.ended) {
          safePause(secondary);
          return;
        }

        const primaryTime = Math.max(0, primary.currentTime);
        if (primaryTime < offset) {
          if (Math.abs(secondary.currentTime) > 0.001) {
            secondary.currentTime = 0;
          }
          pendingSecondaryResume = false;
          safePause(secondary);
          return;
        }

        const targetRaw = primaryTime - offset;
        const secondaryDuration = secondary.duration;
        if (
          Number.isFinite(secondaryDuration) &&
          secondaryDuration > 0 &&
          targetRaw >= secondaryDuration - 0.001
        ) {
          secondary.currentTime = Math.max(0, secondaryDuration - 0.001);
          pendingSecondaryResume = false;
          safePause(secondary);
          return;
        }

        const targetTime = clampTime(targetRaw, secondary);
        const drift = secondary.currentTime - targetTime;

        if (Math.abs(drift) > SEEK_THRESHOLD_SECONDS) {
          secondary.currentTime = targetTime;
          pendingSecondaryResume = true;
          safePause(secondary);
          log("secondary-resync", {
            reason,
            drift: round3(drift),
            primaryTime: round3(primaryTime),
            secondaryTime: round3(secondary.currentTime),
            targetTime: round3(targetTime),
          });
          return;
        }

        pendingSecondaryResume = false;
        if (secondary.paused) {
          safePlay(secondary);
        }
      };

      const maybeStart = () => {
        const primaryReady = primary.readyState >= HTMLMediaElement.HAVE_FUTURE_DATA;
        const secondaryReady = secondary.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA;
        if (!primaryReady || !secondaryReady) return;

        if (!hasStarted) {
          hasStarted = true;
          primary.loop = false;
          secondary.loop = false;
          primary.playbackRate = 1;
          secondary.playbackRate = 1;
          resetForLoop();
          safePlay(primary);
          sync("start");
          startLoop();
          return;
        }

        sync("ready");
        if (!primary.paused) {
          startLoop();
        }
      };

      const handlePrimaryPlay = () => {
        setIsPaused(false);
        sync("primary-play");
        startLoop();
      };

      const handlePrimaryPause = () => {
        setIsPaused(true);
        stopLoop();
        safePause(secondary);
      };

      const handlePrimaryWaiting = () => {
        safePause(secondary);
      };

      const handlePrimarySeeking = () => {
        safePause(secondary);
      };

      const handlePrimarySeeked = () => {
        sync("primary-seeked");
        if (!primary.paused) {
          startLoop();
        }
      };

      const handlePrimaryEnded = () => {
        stopLoop();
        resetForLoop();
        safePlay(primary);
        sync("primary-ended");
        startLoop();
      };

      const handleSecondarySeeked = () => {
        if (pendingSecondaryResume && !primary.paused && !primary.seeking) {
          pendingSecondaryResume = false;
          safePlay(secondary);
        }
      };

      const handleSecondaryWaiting = () => {
        safePause(secondary);
      };

      const handleSecondaryError = () => {
        const err = secondary.error;
        log("secondary-error", {
          code: err?.code,
          message: err?.message,
        });
      };

      const listeners: {
        target: HTMLMediaElement;
        event: keyof HTMLMediaElementEventMap;
        handler: EventListener;
      }[] = [
        { target: primary, event: "play", handler: handlePrimaryPlay as EventListener },
        { target: primary, event: "pause", handler: handlePrimaryPause as EventListener },
        { target: primary, event: "waiting", handler: handlePrimaryWaiting as EventListener },
        { target: primary, event: "seeking", handler: handlePrimarySeeking as EventListener },
        { target: primary, event: "seeked", handler: handlePrimarySeeked as EventListener },
        { target: primary, event: "ended", handler: handlePrimaryEnded as EventListener },
        { target: primary, event: "canplay", handler: maybeStart as EventListener },
        { target: secondary, event: "canplay", handler: maybeStart as EventListener },
        { target: secondary, event: "seeked", handler: handleSecondarySeeked as EventListener },
        { target: secondary, event: "waiting", handler: handleSecondaryWaiting as EventListener },
        { target: secondary, event: "error", handler: handleSecondaryError as EventListener },
      ];

      for (const { target, event, handler } of listeners) {
        target.addEventListener(event, handler);
      }

      primary.load();
      secondary.load();
      setIsPaused(primary.paused);
      maybeStart();

      innerCleanup = () => {
        stopLoop();
        for (const { target, event, handler } of listeners) {
          target.removeEventListener(event, handler);
        }
      };
    };

    run();

    return () => {
      if (retryRafId !== null) {
        cancelAnimationFrame(retryRafId);
      }
      innerCleanup?.();
    };
  }, [enabled, primaryRef, secondaryRef, startOffsetSeconds, ...deps]);

  return { togglePlayPause, isPaused };
}
