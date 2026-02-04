import { type DependencyList, type RefObject, useEffect } from "react";

const DEBUG = import.meta.env.DEV;

const MIN_DRIFT_SECONDS = 0.025;
const LOOP_RESET_EPSILON = 0.05;
const FRAME_DURATION_FALLBACK = 1 / 30;
const RESYNC_COOLDOWN_MS = 200;
const HOLD_THRESHOLD_MULTIPLIER = 1.5;
const RESUME_THRESHOLD_MULTIPLIER = 0.75;

type SyncState = "waiting" | "seeking" | "holding" | "running";

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
  deps: DependencyList = []
) {
  useEffect(() => {
    const primary = primaryRef.current;
    const secondary = secondaryRef.current;

    if (!primary || !secondary) return undefined;

    const initAt = performance.now();

    const writeLog = (level: "info" | "debug", event: string, data?: unknown) => {
      if (DEBUG) {
        const now = performance.now();
        const sinceInitMs = Math.round(now - initAt);
        const prefix = `[video-sync] +${String(sinceInitMs)}ms ${event}`;
        if (level === "info") {
          console.info(prefix, data);
        } else {
          console.debug(prefix, data);
        }
      }
    };

    const logInfo = (event: string, data?: unknown) => {
      writeLog("info", event, data);
    };
    const logDebug = (event: string, data?: unknown) => {
      writeLog("debug", event, data);
    };

    let frameCallbackId: number | null = null;
    let rafId: number | null = null;
    let isLooping = false;
    let lastPrimaryTime = 0;
    const primaryFrame = {
      lastMediaTime: null as number | null,
      estimate: FRAME_DURATION_FALLBACK,
    };
    const secondaryFrame = {
      lastMediaTime: null as number | null,
      estimate: FRAME_DURATION_FALLBACK,
    };
    let syncState: SyncState = "waiting";
    let hasStarted = false;
    let lastResyncAt = 0;
    let secondaryFrameCallbackId: number | null = null;

    const offset = Number.isFinite(startOffsetSeconds) ? Math.max(0, startOffsetSeconds) : 0;

    const getPrimaryLoopStart = () => {
      if (offset <= 0) return 0;
      const duration = primary.duration;
      if (Number.isFinite(duration) && duration > 0 && offset >= duration) {
        return 0;
      }
      return clampTime(offset, primary);
    };

    const seekPrimaryToOffset = (reason: "start" | "loop") => {
      const loopStart = getPrimaryLoopStart();
      if (loopStart <= 0) return false;
      if (primary.readyState < HTMLMediaElement.HAVE_METADATA) return false;
      if (Math.abs(primary.currentTime - loopStart) < 0.001) return false;
      primary.currentTime = loopStart;
      logDebug("primary-offset-seek", {
        reason,
        startTime: round3(loopStart),
      });
      return true;
    };

    // Track mediaTime deltas to estimate frame duration for drift thresholds.
    const updateFrameEstimate = (
      frameState: { lastMediaTime: number | null; estimate: number },
      mediaTime: number
    ) => {
      if (frameState.lastMediaTime === null) {
        frameState.lastMediaTime = mediaTime;
        return;
      }

      const delta = mediaTime - frameState.lastMediaTime;
      frameState.lastMediaTime = mediaTime;

      if (delta > 0.001 && delta < 0.5) {
        frameState.estimate = delta;
      }
    };

    // Keep secondary paused at normal rate to avoid rate drift.
    const pauseSecondary = () => {
      secondary.playbackRate = 1;
      safePause(secondary);
    };

    // Waiting means secondary is paused and ready to align on next frame.
    const setWaiting = (resetTime: boolean) => {
      syncState = "waiting";
      if (
        resetTime &&
        secondary.readyState >= HTMLMediaElement.HAVE_METADATA &&
        secondary.currentTime !== 0
      ) {
        secondary.currentTime = 0;
      }
      pauseSecondary();
    };

    // Hard seek to a target time and enter seeking state.
    const seekSecondaryTo = (targetTime: number, now: number) => {
      const clamped = clampTime(targetTime, secondary);
      const delta = Math.abs(secondary.currentTime - clamped);
      if (delta < 0.001 && !secondary.seeking) {
        syncState = "running";
        return false;
      }
      secondary.currentTime = clamped;
      syncState = "seeking";
      lastResyncAt = now;
      return true;
    };

    // Main sync loop: keeps secondary aligned to primary minus offset.
    const sync = (primaryTime: number, mediaTime?: number) => {
      const now = performance.now();

      if (typeof mediaTime === "number" && Number.isFinite(mediaTime)) {
        updateFrameEstimate(primaryFrame, mediaTime);
      }

      const sanitizedPrimaryTime = Math.max(0, primaryTime);
      const primaryDuration = primary.duration;
      const hasPrimaryDuration = Number.isFinite(primaryDuration) && primaryDuration > 0;
      const loopThreshold = Math.max(primaryFrame.estimate, FRAME_DURATION_FALLBACK);

      if (hasPrimaryDuration && primaryDuration - sanitizedPrimaryTime <= loopThreshold) {
        setWaiting(true);
        primaryFrame.lastMediaTime = null;
        const didSeek = seekPrimaryToOffset("loop");
        if (didSeek) {
          safePlay(primary);
        }
        lastPrimaryTime = getPrimaryLoopStart();
        logInfo("primary-loop-detected", {
          lastPrimaryTime: round3(lastPrimaryTime),
          currentPrimaryTime: round3(sanitizedPrimaryTime),
        });
        return;
      }

      // Detect looping fallback when duration is unknown.
      const looped =
        !hasPrimaryDuration && sanitizedPrimaryTime + LOOP_RESET_EPSILON < lastPrimaryTime;

      if (looped) {
        setWaiting(true);
        primaryFrame.lastMediaTime = null;
        const didSeek = seekPrimaryToOffset("loop");
        if (didSeek) {
          safePlay(primary);
        }
        lastPrimaryTime = getPrimaryLoopStart();
        logInfo("primary-loop-detected", {
          lastPrimaryTime: round3(lastPrimaryTime),
          currentPrimaryTime: round3(sanitizedPrimaryTime),
        });
        return;
      }

      lastPrimaryTime = sanitizedPrimaryTime;

      // If primary isn't advancing, keep secondary paused.
      if (primary.paused || primary.seeking) {
        pauseSecondary();
        logDebug("primary-not-running", {
          paused: primary.paused,
          seeking: primary.seeking,
        });
        return;
      }

      // Avoid fighting browser seeks.
      if (secondary.seeking || syncState === "seeking") {
        logDebug("secondary-seeking-or-cooldown", {
          seeking: secondary.seeking,
          state: syncState,
        });
        return;
      }

      // Hold secondary at 0 until primary passes the offset.
      if (sanitizedPrimaryTime < offset) {
        setWaiting(true);
        logDebug("secondary-hold-waiting-for-offset", {
          primaryTime: round3(sanitizedPrimaryTime),
          offset: round3(offset),
        });
        return;
      }

      const targetTime = sanitizedPrimaryTime - offset;
      const duration = secondary.duration;

      // If secondary would exceed its duration, hold at the last frame.
      if (Number.isFinite(duration) && duration > 0 && targetTime >= duration) {
        secondary.currentTime = Math.max(0, duration - 0.001);
        syncState = "holding";
        pauseSecondary();
        logInfo("secondary-ended-hold", {
          targetTime: round3(targetTime),
          secondaryDuration: round3(duration),
        });
        return;
      }

      if (secondary.readyState < HTMLMediaElement.HAVE_CURRENT_DATA) {
        logDebug("secondary-not-ready", {
          readyState: secondary.readyState,
        });
        return;
      }

      // First alignment after start/loop.
      if (syncState === "waiting") {
        const didSeek = seekSecondaryTo(targetTime, now);
        logInfo("secondary-start-align", {
          targetTime: round3(targetTime),
          secondaryCurrentTime: round3(secondary.currentTime),
        });
        if (didSeek) return;
      }

      // Positive drift means secondary is ahead of target.
      const drift = secondary.currentTime - targetTime;
      const maxFrameDuration = Math.max(primaryFrame.estimate, secondaryFrame.estimate);
      const allowedDrift = Math.max(maxFrameDuration * 1.5, MIN_DRIFT_SECONDS);
      const holdThreshold = allowedDrift * HOLD_THRESHOLD_MULTIPLIER;
      const resumeThreshold = allowedDrift * RESUME_THRESHOLD_MULTIPLIER;
      const canHardResync = lastResyncAt === 0 || now - lastResyncAt >= RESYNC_COOLDOWN_MS;

      // If ahead too far, pause until primary catches up.
      if (drift > holdThreshold) {
        if (syncState !== "holding") {
          syncState = "holding";
          pauseSecondary();
          logDebug("secondary-hold-drift", {
            drift: round3(drift),
            allowedDrift: round3(allowedDrift),
            holdThreshold: round3(holdThreshold),
          });
        }
        // Resume once drift falls back under the resume threshold.
      } else if (drift < resumeThreshold) {
        if (syncState === "holding") {
          syncState = "running";
          logDebug("secondary-resume-drift", {
            drift: round3(drift),
            allowedDrift: round3(allowedDrift),
            resumeThreshold: round3(resumeThreshold),
          });
        }
      }

      // If behind too far, seek to catch up (rate correction would be jittery).
      if (drift < -holdThreshold) {
        if (canHardResync) {
          const didSeek = seekSecondaryTo(targetTime, now);
          logDebug("hard-resync", {
            drift: round3(drift),
            holdThreshold: round3(holdThreshold),
            targetTime: round3(targetTime),
            secondaryCurrentTime: round3(secondary.currentTime),
          });
          if (didSeek) return;
        } else {
          logDebug("hard-resync-suppressed", {
            drift: round3(drift),
            holdThreshold: round3(holdThreshold),
            cooldownMs: Math.round(now - lastResyncAt),
          });
        }
      }

      if (secondary.paused && syncState === "running") {
        safePlay(secondary);
        logDebug("secondary-play", {
          secondaryCurrentTime: round3(secondary.currentTime),
        });
      }
    };

    // Track secondary frame timing when supported.
    const startSecondaryFrameLoop = () => {
      if (typeof secondary.requestVideoFrameCallback !== "function") {
        return;
      }

      const onSecondaryFrame: VideoFrameRequestCallback = (_now, metadata) => {
        if (typeof metadata.mediaTime === "number") {
          updateFrameEstimate(secondaryFrame, metadata.mediaTime);
        }
        secondaryFrameCallbackId = secondary.requestVideoFrameCallback(onSecondaryFrame);
      };

      secondaryFrameCallbackId = secondary.requestVideoFrameCallback(onSecondaryFrame);
    };

    const stopSecondaryFrameLoop = () => {
      if (
        secondaryFrameCallbackId !== null &&
        typeof secondary.cancelVideoFrameCallback === "function"
      ) {
        secondary.cancelVideoFrameCallback(secondaryFrameCallbackId);
        secondaryFrameCallbackId = null;
      }
    };

    // Use RVFC when available for frame-accurate timing.
    const startLoop = () => {
      if (isLooping) return;
      isLooping = true;

      if (typeof primary.requestVideoFrameCallback === "function") {
        logInfo("sync-loop-start", { mode: "rvfc" });
        const onFrame: VideoFrameRequestCallback = (_now, metadata) => {
          sync(primary.currentTime, metadata.mediaTime);
          frameCallbackId = primary.requestVideoFrameCallback(onFrame);
        };

        frameCallbackId = primary.requestVideoFrameCallback(onFrame);
        return;
      }

      logInfo("sync-loop-start", { mode: "raf" });
      const onAnimationFrame = () => {
        sync(primary.currentTime);
        rafId = requestAnimationFrame(onAnimationFrame);
      };

      rafId = requestAnimationFrame(onAnimationFrame);
    };

    const stopLoop = () => {
      isLooping = false;
      logInfo("sync-loop-stop");
      if (frameCallbackId !== null && typeof primary.cancelVideoFrameCallback === "function") {
        primary.cancelVideoFrameCallback(frameCallbackId);
        frameCallbackId = null;
      }
      if (rafId !== null) {
        cancelAnimationFrame(rafId);
        rafId = null;
      }
    };

    // One-time start: configure looping and kick off sync.
    const startPlayback = () => {
      hasStarted = true;
      primary.loop = false;
      primary.playbackRate = 1;
      secondary.loop = false;
      setWaiting(true);
      startSecondaryFrameLoop();
      logInfo("start", {
        offset: round3(offset),
        primaryCurrentSrc: primary.currentSrc || undefined,
        secondaryCurrentSrc: secondary.currentSrc || undefined,
        primaryPreload: primary.preload,
        secondaryPreload: secondary.preload,
      });
      seekPrimaryToOffset("start");
      safePlay(primary);
      startLoop();
    };

    const handlePrimaryPlay = () => {
      logInfo("primary-play", { t: round3(primary.currentTime) });
      startLoop();
    };

    const handlePrimaryPause = () => {
      logInfo("primary-pause", { t: round3(primary.currentTime) });
      stopLoop();
      pauseSecondary();
    };

    const handlePrimaryWaiting = () => {
      logInfo("primary-waiting", { t: round3(primary.currentTime) });
      pauseSecondary();
    };

    const handlePrimarySeeking = () => {
      logInfo("primary-seeking", {
        from: round3(primary.currentTime),
      });
      primaryFrame.lastMediaTime = null;
      setWaiting(false);
    };

    const handlePrimarySeeked = () => {
      logInfo("primary-seeked", { to: round3(primary.currentTime) });
      sync(primary.currentTime);
      if (!primary.paused) {
        startLoop();
      }
    };

    const handlePrimaryEnded = () => {
      logInfo("primary-ended");
      setWaiting(true);
      const didSeek = seekPrimaryToOffset("loop");
      if (didSeek) {
        safePlay(primary);
      }
    };

    const maybeStart = () => {
      if (
        primary.readyState >= HTMLMediaElement.HAVE_FUTURE_DATA &&
        secondary.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA
      ) {
        logInfo("canplay-both", {
          primaryReadyState: primary.readyState,
          secondaryReadyState: secondary.readyState,
        });
        if (!hasStarted) {
          startPlayback();
        } else if (!primary.paused) {
          sync(primary.currentTime);
          startLoop();
        }
      } else {
        logDebug("canplay-insufficient-data", {
          primaryReadyState: primary.readyState,
          secondaryReadyState: secondary.readyState,
        });
      }
    };

    const handleSecondaryWaiting = () => {
      logInfo("secondary-waiting", {
        t: round3(secondary.currentTime),
      });
    };

    const handleSecondaryStalled = () => {
      logInfo("secondary-stalled", {
        t: round3(secondary.currentTime),
      });
    };

    const handleSecondarySeeking = () => {
      syncState = "seeking";
      logDebug("secondary-seeking", {
        t: round3(secondary.currentTime),
      });
    };

    const handleSecondarySeeked = () => {
      syncState = "running";
      lastResyncAt = performance.now();
      logDebug("secondary-seeked", {
        t: round3(secondary.currentTime),
      });
    };

    const handleSecondaryError = () => {
      const err = secondary.error;
      logInfo("secondary-error", {
        code: err?.code,
        message: err?.message,
      });
    };

    const listeners: {
      target: HTMLMediaElement;
      event: keyof HTMLMediaElementEventMap;
      handler: EventListener;
    }[] = [
      {
        target: primary,
        event: "play",
        handler: handlePrimaryPlay as EventListener,
      },
      {
        target: primary,
        event: "pause",
        handler: handlePrimaryPause as EventListener,
      },
      {
        target: primary,
        event: "waiting",
        handler: handlePrimaryWaiting as EventListener,
      },
      {
        target: primary,
        event: "seeking",
        handler: handlePrimarySeeking as EventListener,
      },
      {
        target: primary,
        event: "seeked",
        handler: handlePrimarySeeked as EventListener,
      },
      {
        target: primary,
        event: "ended",
        handler: handlePrimaryEnded as EventListener,
      },
      {
        target: primary,
        event: "canplay",
        handler: maybeStart as EventListener,
      },
      {
        target: secondary,
        event: "canplay",
        handler: maybeStart as EventListener,
      },
      {
        target: secondary,
        event: "waiting",
        handler: handleSecondaryWaiting as EventListener,
      },
      {
        target: secondary,
        event: "stalled",
        handler: handleSecondaryStalled as EventListener,
      },
      {
        target: secondary,
        event: "seeking",
        handler: handleSecondarySeeking as EventListener,
      },
      {
        target: secondary,
        event: "seeked",
        handler: handleSecondarySeeked as EventListener,
      },
      {
        target: secondary,
        event: "error",
        handler: handleSecondaryError as EventListener,
      },
    ];

    for (const { target, event, handler } of listeners) {
      target.addEventListener(event, handler);
    }

    primary.load();
    secondary.load();
    maybeStart();

    return () => {
      logInfo("cleanup");
      stopLoop();
      stopSecondaryFrameLoop();
      for (const { target, event, handler } of listeners) {
        target.removeEventListener(event, handler);
      }
    };
  }, [primaryRef, secondaryRef, startOffsetSeconds, ...deps]);
}
