import { useEffect, useRef } from "react";

import { useCompressionStore, WorkerState } from "@/features/compression/store/compression-store";
import { desktopClient } from "@/platform/desktop/client";
import type {
  MediaJobCompletePayload,
  MediaJobErrorPayload,
  MediaJobProgressPayload,
} from "@/types/native";

function handleProgressUpdate(payload: MediaJobProgressPayload) {
  const s = useCompressionStore.getState();

  if (payload.kind === "preview") {
    if (s.workerState !== WorkerState.GeneratingPreview) return;
    if (s.activePreviewJobId != null && s.activePreviewJobId !== payload.jobId) return;
    useCompressionStore.setState({
      activePreviewJobId: payload.jobId,
      progress: payload.progress,
      progressStep: payload.step ?? null,
    });
    return;
  }

  if (s.workerState !== WorkerState.Transcoding) return;
  if (s.activeTranscodeJobId != null && s.activeTranscodeJobId !== payload.jobId) return;
  useCompressionStore.setState({
    activeTranscodeJobId: payload.jobId,
    progress: payload.progress,
    progressStep: payload.step ?? null,
  });
}

function handleJobError(payload: MediaJobErrorPayload) {
  const { summary, detail, kind, jobId } = payload;
  const s = useCompressionStore.getState();

  if (kind === "preview") {
    if (s.workerState !== WorkerState.GeneratingPreview) return;
    if (s.activePreviewJobId != null && s.activePreviewJobId !== jobId) return;
    if (summary === "Aborted") {
      useCompressionStore.setState({
        workerState: WorkerState.Idle,
        progress: 0,
        progressStep: null,
        activePreviewJobId: null,
      });
      return;
    }
    useCompressionStore.setState({
      workerState: WorkerState.Idle,
      activePreviewJobId: null,
      error: {
        type: "Preview Error",
        message: summary,
        detail,
      },
    });
    return;
  }

  if (s.workerState !== WorkerState.Transcoding) return;
  if (s.activeTranscodeJobId != null && s.activeTranscodeJobId !== jobId) return;
  if (summary === "Aborted") {
    useCompressionStore.setState({
      workerState: WorkerState.Idle,
      progress: 0,
      progressStep: null,
      activeTranscodeJobId: null,
    });
    return;
  }
  useCompressionStore.setState({
    workerState: WorkerState.Idle,
    activeTranscodeJobId: null,
    error: {
      type: "Transcode Error",
      message: summary,
      detail,
    },
  });
}

function handleJobComplete(payload: MediaJobCompletePayload) {
  const s = useCompressionStore.getState();

  if (payload.kind === "preview") {
    if (s.workerState !== WorkerState.GeneratingPreview) return;
    if (s.activePreviewJobId != null && s.activePreviewJobId !== payload.jobId) return;
    useCompressionStore.setState({
      activePreviewJobId: payload.jobId,
      progress: 1,
      progressStep: null,
    });
    return;
  }

  if (s.workerState !== WorkerState.Transcoding) return;
  if (s.activeTranscodeJobId != null && s.activeTranscodeJobId !== payload.jobId) return;
  useCompressionStore.setState({
    activeTranscodeJobId: payload.jobId,
    progress: 1,
    progressStep: null,
  });
}

export function useCompressionStoreInit() {
  const effectIdRef = useRef(0);

  useEffect(() => {
    const effectId = ++effectIdRef.current;
    let cancelled = false;
    const unlisteners: (() => void)[] = [];

    const setup = async () => {
      try {
        await useCompressionStore.getState().initBuildVariant();
        if (useCompressionStore.getState().initError) {
          return;
        }

        const [unProgress, unError, unComplete, unOpenFile, unMenuOpenFile] = await Promise.all([
          desktopClient.listen<MediaJobProgressPayload>("media.job.progress", (payload) => {
            handleProgressUpdate(payload);
          }),
          desktopClient.listen<MediaJobErrorPayload>("media.job.error", (payload) => {
            handleJobError(payload);
          }),
          desktopClient.listen<MediaJobCompletePayload>("media.job.complete", (payload) => {
            handleJobComplete(payload);
          }),
          desktopClient.listen<string[]>("open-file", (paths) => {
            if (Array.isArray(paths) && paths.length > 0) {
              void useCompressionStore.getState().selectPath(paths[0]);
            }
          }),
          desktopClient.listen("menu-open-file", () => {
            void useCompressionStore.getState().browseAndSelectFile();
          }),
        ]);
        if (cancelled || effectId !== effectIdRef.current) {
          unProgress();
          unError();
          unComplete();
          unOpenFile();
          unMenuOpenFile();
          return;
        }
        unlisteners.push(unProgress, unError, unComplete, unOpenFile, unMenuOpenFile);
        useCompressionStore.setState({ listenersReady: true });

        const pendingPaths = await desktopClient.invoke("get_pending_opened_files");
        if (pendingPaths.length > 0) {
          void useCompressionStore.getState().selectPath(pendingPaths[0]);
        }
      } catch (error) {
        if (cancelled || effectId !== effectIdRef.current) {
          return;
        }
        const message = error instanceof Error ? error.message : String(error);
        useCompressionStore.setState({
          initError: message,
          listenersReady: false,
        });
      }
    };

    void setup();

    return () => {
      cancelled = true;
      useCompressionStore.setState({ listenersReady: false });
      unlisteners.forEach((u) => {
        u();
      });
    };
  }, []);
}
