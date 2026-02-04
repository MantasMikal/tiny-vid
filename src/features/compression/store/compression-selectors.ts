import {
  type CompressionState,
  WorkerState,
} from "@/features/compression/store/compression-store";

export const selectIsInitialized = (s: CompressionState): boolean =>
  s.availableCodecs.length > 0 && s.listenersReady;

export const selectIsActionsDisabled = (s: CompressionState): boolean =>
  !s.inputPath ||
  s.compressionOptions == null ||
  s.isSaving ||
  s.workerState === WorkerState.Transcoding;
