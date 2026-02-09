import type { RateControlMode } from "@/features/compression/lib/compression-options";
import { supportsTwoPassCodec } from "@/features/compression/lib/compression-options";

export const TARGET_SIZE_OVERHEAD_RATIO = 0.02;
export const TARGET_SIZE_MIN_VIDEO_KBPS = 200;
export const TARGET_SIZE_MAX_VIDEO_KBPS = 100_000;
export const TARGET_SIZE_SUPPORT_MESSAGE = "Target size requires libx264, libx265, or libvpx-vp9.";

const BYTES_PER_MB = 1024 * 1024;

export interface TargetSizeComputationInput {
  targetSizeMb?: number;
  durationSecs?: number;
  removeAudio?: boolean;
  audioBitrateKbps?: number;
  audioStreamCount?: number;
  preserveAdditionalAudioStreams?: boolean;
}

export interface TargetSizeComputationResult {
  ok: boolean;
  videoBitrateKbps?: number;
  audioBitrateKbpsTotal?: number;
  audioSizeMb?: number;
  estimatedSizeMb?: number;
  clamped?: boolean;
  error?: string;
}

export interface TargetSizeSupport {
  supported: boolean;
  reason?: string;
}

export interface TargetSizeStatusInput extends TargetSizeComputationInput {
  rateControlMode?: RateControlMode;
  requireDuration?: boolean;
}

export interface TargetSizeStatus {
  isActive: boolean;
  canCompute: boolean;
  result: TargetSizeComputationResult | null;
  error: string | null;
}

function resolveAudioStreamCount(
  input: Pick<
    TargetSizeComputationInput,
    "removeAudio" | "audioStreamCount" | "preserveAdditionalAudioStreams"
  >
): number {
  if (input.removeAudio) return 0;
  const count = input.audioStreamCount ?? 1;
  if (count <= 0) return 0;
  if (input.preserveAdditionalAudioStreams) return count;
  return 1;
}

export function computeTargetVideoBitrateKbps(
  input: TargetSizeComputationInput
): TargetSizeComputationResult {
  const targetSizeMb = input.targetSizeMb;
  if (!Number.isFinite(targetSizeMb) || (targetSizeMb ?? 0) <= 0) {
    return {
      ok: false,
      error: "Enter a target size.",
    };
  }

  const durationSecs = input.durationSecs;
  if (!Number.isFinite(durationSecs) || (durationSecs ?? 0) <= 0) {
    return {
      ok: false,
      error: "Video duration is required for target size mode.",
    };
  }

  const audioStreamCount = resolveAudioStreamCount(input);
  const audioBitrateKbps = input.audioBitrateKbps ?? 128;
  const audioBitrateKbpsTotal = audioStreamCount * audioBitrateKbps;

  const totalBits = (targetSizeMb ?? 0) * BYTES_PER_MB * 8;
  const overheadBits = totalBits * TARGET_SIZE_OVERHEAD_RATIO;
  const audioBits = audioBitrateKbpsTotal * 1000 * (durationSecs ?? 0);
  const videoBits = totalBits - overheadBits - audioBits;

  const audioSizeMb = audioBits / 8 / BYTES_PER_MB;

  if (!Number.isFinite(videoBits) || videoBits <= 0) {
    return {
      ok: false,
      audioBitrateKbpsTotal,
      audioSizeMb,
      error: "Target size is too small for audio.",
    };
  }

  const rawVideoKbps = Math.floor(videoBits / (durationSecs ?? 1) / 1000);
  let clamped = false;
  let videoBitrateKbps = rawVideoKbps;
  if (rawVideoKbps < TARGET_SIZE_MIN_VIDEO_KBPS) {
    videoBitrateKbps = TARGET_SIZE_MIN_VIDEO_KBPS;
    clamped = true;
  } else if (rawVideoKbps > TARGET_SIZE_MAX_VIDEO_KBPS) {
    videoBitrateKbps = TARGET_SIZE_MAX_VIDEO_KBPS;
    clamped = true;
  }

  const totalBitsEstimate =
    ((videoBitrateKbps + audioBitrateKbpsTotal) * 1000 * (durationSecs ?? 0)) /
    (1 - TARGET_SIZE_OVERHEAD_RATIO);
  const estimatedSizeMb = totalBitsEstimate / 8 / BYTES_PER_MB;

  return {
    ok: true,
    videoBitrateKbps,
    audioBitrateKbpsTotal,
    audioSizeMb,
    estimatedSizeMb,
    clamped,
  };
}

export function getTargetSizeSupport(codec?: string | null): TargetSizeSupport {
  if (!codec) {
    return { supported: false, reason: TARGET_SIZE_SUPPORT_MESSAGE };
  }
  const supported = supportsTwoPassCodec(codec);
  return supported ? { supported } : { supported, reason: TARGET_SIZE_SUPPORT_MESSAGE };
}

export function formatTargetSizeError(result: TargetSizeComputationResult | null): string | null {
  if (!result || result.ok) return null;
  if (result.audioSizeMb != null && result.error === "Target size is too small for audio.") {
    return `${result.error} (Audio ≈ ${result.audioSizeMb.toFixed(2)} MB)`;
  }
  return result.error ?? "Enter a target size.";
}

export function getTargetSizeStatus(input: TargetSizeStatusInput): TargetSizeStatus {
  const isActive = input.rateControlMode === "targetSize";
  const durationSecs = input.durationSecs;
  const canCompute = Number.isFinite(durationSecs) && (durationSecs ?? 0) > 0;
  if (!isActive) {
    return { isActive, canCompute, result: null, error: null };
  }
  if (!canCompute) {
    return {
      isActive,
      canCompute,
      result: null,
      error: input.requireDuration ? "Video duration is required for target size mode." : null,
    };
  }
  const result = computeTargetVideoBitrateKbps(input);
  const error = formatTargetSizeError(result);
  return { isActive, canCompute, result, error };
}
