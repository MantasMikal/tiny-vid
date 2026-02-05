import { type DependencyList, type RefObject, useEffect } from "react";

const DEBUG = import.meta.env.DEV;

const MIN_DRIFT_SECONDS = 0.1; // Minimum drift (seconds) before we consider videos out of sync
const SYNC_INTERVAL_MS = 75; // Ms between sync checks; wall-clock based,
const HOLD_THRESHOLD_MULTIPLIER = 1.2; // Secondary is paused when drift exceeds allowedDrift × this;
const BEHIND_RESYNC_THRESHOLD_SECONDS = 0.03; // Seek to catch up when secondary is behind by more than this
const PRIMING_THRESHOLD_SECONDS = 0.15; // Seconds to wait for primary to advance; only used after we've seen a hold (heavy decode).

enum SyncState {
  Waiting = "waiting",
  Seeking = "seeking",
  Holding = "holding",
  Running = "running",
}

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
) {
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

      /** Snapshot for debugging: primary/secondary times, target, drift, state. */
      const logStateSnapshot = (event: string, extra?: Record<string, unknown>) => {
        if (!DEBUG) return;
        const pt = primary.currentTime;
        const st = secondary.currentTime;
        const target = Math.max(0, pt - offset);
        const drift = st - target;
        writeLog("debug", `state@${event}`, {
          primaryT: round3(pt),
          secondaryT: round3(st),
          targetT: round3(target),
          drift: round3(drift),
          syncState,
          primaryPaused: primary.paused,
          primarySeeking: primary.seeking,
          secondarySeeking: secondary.seeking,
          hasEverHeld,
          ...extra,
        });
      };

      let intervalId: ReturnType<typeof setInterval> | null = null;
      let isLooping = false;
      let syncState = SyncState.Waiting;
      let hasStarted = false;
      let lastResyncAt = 0;
      /** When true, handleSecondarySeeked will not set lastResyncAt; used for priming align so we can correct immediately if behind. */
      let skipNextResyncCooldown = false;
      /** True after we've entered holding; enables priming on subsequent loops for heavy-decode videos. */
      let hasEverHeld = false;
      let lastDriftLogAt = 0;

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
        if (loopStart < 0) return false;
        if (primary.readyState < HTMLMediaElement.HAVE_METADATA) return false;
        const from = primary.currentTime;
        if (Math.abs(from - loopStart) < 0.001) return false;
        primary.currentTime = loopStart;
        logDebug("primary-offset-seek", {
          reason,
          from: round3(from),
          to: round3(loopStart),
          secondaryTime: round3(secondary.currentTime),
        });
        return true;
      };

      // Keep secondary paused at normal rate to avoid rate drift.
      const pauseSecondary = () => {
        secondary.playbackRate = 1;
        safePause(secondary);
      };

      // Waiting means secondary is paused and ready to align on next frame.
      const setWaiting = (resetTime: boolean) => {
        syncState = SyncState.Waiting;
        if (
          resetTime &&
          secondary.readyState >= HTMLMediaElement.HAVE_METADATA &&
          secondary.currentTime !== 0
        ) {
          logDebug("setWaiting-reset-secondary", {
            resetTime,
            secondaryBefore: round3(secondary.currentTime),
            secondaryAfter: 0,
          });
          secondary.currentTime = 0;
        }
        pauseSecondary();
      };

      // Hard seek to a target time and enter seeking state.
      const seekSecondaryTo = (targetTime: number, now: number, skipCooldown = false) => {
        const clamped = clampTime(targetTime, secondary);
        const delta = Math.abs(secondary.currentTime - clamped);
        if (delta < 0.001 && !secondary.seeking) {
          syncState = SyncState.Running;
          return false;
        }
        secondary.currentTime = clamped;
        syncState = SyncState.Seeking;
        if (!skipCooldown) lastResyncAt = now;
        return true;
      };

      // Main sync loop: keeps secondary aligned to primary minus offset.
      const sync = (primaryTime: number) => {
        const now = performance.now();
        const sanitizedPrimaryTime = Math.max(0, primaryTime);
        const primaryDuration = primary.duration;
        const hasPrimaryDuration = Number.isFinite(primaryDuration) && primaryDuration > 0;
        const loopThreshold = SYNC_INTERVAL_MS / 1000;

        if (hasPrimaryDuration && primaryDuration - sanitizedPrimaryTime <= loopThreshold) {
          logInfo("primary-loop-detected", {
            primaryTime: round3(sanitizedPrimaryTime),
            primaryDuration: round3(primaryDuration),
            loopThreshold: round3(loopThreshold),
            distanceFromEnd: round3(primaryDuration - sanitizedPrimaryTime),
            secondaryTimeBeforeReset: round3(secondary.currentTime),
          });
          setWaiting(true);
          const didSeek = seekPrimaryToOffset("loop");
          if (didSeek) {
            safePlay(primary);
          }
          logStateSnapshot("after-loop-detect", { didSeek });
          return;
        }

        // If primary isn't advancing, keep secondary paused.
        if (primary.paused || primary.seeking) {
          pauseSecondary();
          if (primary.paused) {
            stopLoop();
          }
          logDebug("primary-not-running", {
            paused: primary.paused,
            seeking: primary.seeking,
          });
          return;
        }

        // Avoid fighting browser seeks.
        if (secondary.seeking || syncState === SyncState.Seeking) {
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
          const holdAt = Math.max(0, duration - 0.001);
          secondary.currentTime = holdAt;
          syncState = SyncState.Holding;
          pauseSecondary();
          logInfo("secondary-ended-hold", {
            targetTime: round3(targetTime),
            secondaryDuration: round3(duration),
            primaryTime: round3(sanitizedPrimaryTime),
            holdingSecondaryAt: round3(holdAt),
          });
          logStateSnapshot("secondary-ended-hold");
          return;
        }

        if (secondary.readyState < HTMLMediaElement.HAVE_CURRENT_DATA) {
          logDebug("secondary-not-ready", {
            readyState: secondary.readyState,
          });
          return;
        }

        // First alignment after start/loop. For heavy-decode videos, wait for primary to advance.
        if (syncState === SyncState.Waiting) {
          const loopStart = getPrimaryLoopStart();
          const primaryAdvance = sanitizedPrimaryTime - loopStart;
          const usePriming = hasEverHeld && primaryAdvance < PRIMING_THRESHOLD_SECONDS;
          if (usePriming) {
            logDebug("priming-wait", {
              primaryAdvance: round3(primaryAdvance),
              threshold: PRIMING_THRESHOLD_SECONDS,
              primaryTime: round3(sanitizedPrimaryTime),
              targetTimeWhenReady: round3(targetTime),
              secondaryTime: round3(secondary.currentTime),
            });
            return;
          }
          const didSeek = seekSecondaryTo(targetTime, now, true);
          if (didSeek) skipNextResyncCooldown = true;
          logInfo("secondary-start-align", {
            targetTime: round3(targetTime),
            secondaryCurrentTime: round3(secondary.currentTime),
            primaryTime: round3(sanitizedPrimaryTime),
            primaryAdvance: round3(primaryAdvance),
            didSeek,
            skipNextResyncCooldown: didSeek,
          });
          logStateSnapshot("after-start-align");
          if (didSeek) return;
        }

        // Positive drift means secondary is ahead of target.
        const drift = secondary.currentTime - targetTime;
        const allowedDrift = MIN_DRIFT_SECONDS;
        const holdThreshold = MIN_DRIFT_SECONDS * HOLD_THRESHOLD_MULTIPLIER;
        const canHardResync = lastResyncAt === 0 || now - lastResyncAt >= SYNC_INTERVAL_MS;

        if (now - lastDriftLogAt >= 200) {
          lastDriftLogAt = now;
          logDebug("drift", {
            drift: round3(drift),
            targetTime: round3(targetTime),
            secondaryTime: round3(secondary.currentTime),
            primaryTime: round3(sanitizedPrimaryTime),
            syncState,
          });
        }

        // If ahead too far, pause until primary catches up.
        if (drift > holdThreshold) {
          if (syncState !== SyncState.Holding) {
            syncState = SyncState.Holding;
            hasEverHeld = true;
            pauseSecondary();
            logDebug("secondary-hold-drift", {
              drift: round3(drift),
              allowedDrift: round3(allowedDrift),
              holdThreshold: round3(holdThreshold),
              primaryTime: round3(sanitizedPrimaryTime),
              targetTime: round3(targetTime),
              secondaryTime: round3(secondary.currentTime),
            });
            logStateSnapshot("after-hold-drift");
          }
          // Resume only when primary has caught up (drift <= 0) to avoid hold/resume thrashing.
        } else if (drift <= 0 && syncState === SyncState.Holding) {
          syncState = SyncState.Running;
          logDebug("secondary-resume-drift", {
            drift: round3(drift),
            allowedDrift: round3(allowedDrift),
            primaryTime: round3(sanitizedPrimaryTime),
            targetTime: round3(targetTime),
            secondaryTime: round3(secondary.currentTime),
          });
          logStateSnapshot("after-resume-drift");
        }

        // If behind too far, seek to catch up (rate correction would be jittery).
        if (drift <= -BEHIND_RESYNC_THRESHOLD_SECONDS) {
          const cooldownMs = lastResyncAt > 0 ? Math.round(now - lastResyncAt) : 0;
          const cooldownRemainingMs = canHardResync
            ? 0
            : Math.max(0, SYNC_INTERVAL_MS - cooldownMs);
          if (canHardResync) {
            const didSeek = seekSecondaryTo(targetTime, now);
            logDebug("hard-resync", {
              drift: round3(drift),
              behindThreshold: BEHIND_RESYNC_THRESHOLD_SECONDS,
              targetTime: round3(targetTime),
              secondaryCurrentTime: round3(secondary.currentTime),
              primaryTime: round3(sanitizedPrimaryTime),
              didSeek,
            });
            logStateSnapshot("after-hard-resync", { didSeek });
            if (didSeek) return;
          } else {
            logDebug("hard-resync-suppressed", {
              drift: round3(drift),
              behindThreshold: BEHIND_RESYNC_THRESHOLD_SECONDS,
              cooldownMs,
              cooldownRemainingMs,
              lastResyncAt: lastResyncAt > 0 ? "set" : "never",
            });
          }
        }

        if (secondary.paused && syncState === SyncState.Running) {
          safePlay(secondary);
          logDebug("secondary-play", {
            secondaryCurrentTime: round3(secondary.currentTime),
          });
        }
      };

      // Time-based sync loop; independent of video FPS.
      const startLoop = () => {
        if (isLooping) return;
        isLooping = true;
        logInfo("sync-loop-start", { mode: "interval", intervalMs: SYNC_INTERVAL_MS });
        intervalId = setInterval(() => {
          sync(primary.currentTime);
        }, SYNC_INTERVAL_MS);
      };

      const stopLoop = () => {
        isLooping = false;
        logInfo("sync-loop-stop");
        if (intervalId !== null) {
          clearInterval(intervalId);
          intervalId = null;
        }
      };

      // One-time start: configure looping and kick off sync.
      const startPlayback = () => {
        hasStarted = true;
        primary.loop = false;
        primary.playbackRate = 1;
        secondary.loop = false;
        setWaiting(true);
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
          secondaryTime: round3(secondary.currentTime),
          syncState,
        });
        // Don't overwrite Waiting - the seek is from our seekPrimaryToOffset (loop); we need priming.
        if (syncState === SyncState.Waiting) {
          pauseSecondary();
          return;
        }
        setWaiting(false);
      };

      const handlePrimarySeeked = () => {
        logInfo("primary-seeked", {
          to: round3(primary.currentTime),
          secondaryTime: round3(secondary.currentTime),
          syncState,
        });
        logStateSnapshot("primary-seeked");
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

      const handleSecondarySeeking = () => {
        // Don't overwrite Waiting - the seek is from our reset; we need priming check in sync().
        if (syncState !== SyncState.Waiting) syncState = SyncState.Seeking;
        logDebug("secondary-seeking", {
          t: round3(secondary.currentTime),
          syncState,
          primaryTime: round3(primary.currentTime),
        });
      };

      const handleSecondarySeeked = () => {
        // Don't overwrite Waiting - stay in waiting for priming check in sync().
        if (syncState !== SyncState.Waiting) syncState = SyncState.Running;
        const skippedCooldown = skipNextResyncCooldown;
        if (!skipNextResyncCooldown) {
          lastResyncAt = performance.now();
        } else {
          skipNextResyncCooldown = false;
        }
        logDebug("secondary-seeked", {
          t: round3(secondary.currentTime),
          syncState,
          primaryTime: round3(primary.currentTime),
          skippedCooldown: skippedCooldown,
        });
        logStateSnapshot("secondary-seeked", { skippedCooldown: skippedCooldown });
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

      innerCleanup = () => {
        logInfo("cleanup");
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
}
