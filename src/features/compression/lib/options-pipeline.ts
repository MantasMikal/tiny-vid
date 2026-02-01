import {
  type Codec,
  type CompressionOptions,
  convertQualityForCodecSwitch,
  type Format,
  getAvailableFormats,
  getCodecInfo,
  getCodecsForFormat,
  getFormatCapabilities,
} from "@/features/compression/lib/compression-options";
import type { CodecInfo } from "@/types/tauri";

const DEFAULT_OPTIONS: CompressionOptions = {
  quality: 75,
  preset: "fast",
  fps: 30,
  scale: 1,
  removeAudio: false,
  codec: "libx264",
  outputFormat: "mp4",
  generatePreview: true,
  previewDuration: 3,
  tune: undefined,
};

const MP4 = "mp4" satisfies Format;

export const BASIC_PRESETS = {
  basic: {
    quality: 90,
    scale: 1,
    removeAudio: false,
    outputFormat: MP4,
    generatePreview: true,
  },
  super: {
    quality: 75,
    scale: 1,
    removeAudio: false,
    outputFormat: MP4,
    generatePreview: true,
  },
  ultra: {
    quality: 60,
    scale: 1,
    removeAudio: false,
    outputFormat: MP4,
    generatePreview: true,
  },
  cooked: {
    quality: 40,
    scale: 1,
    removeAudio: false,
    outputFormat: MP4,
    generatePreview: true,
  },
} as const;

export type BasicPresetId = keyof typeof BASIC_PRESETS;

export const BASIC_PRESET_IDS = ["basic", "super", "ultra", "cooked"] as const;

export const DEFAULT_PRESET_ID: BasicPresetId = "super";

export function isBasicPresetId(s: string): s is BasicPresetId {
  return BASIC_PRESET_IDS.includes(s as BasicPresetId);
}

export function resolve(
  partial: Partial<CompressionOptions>,
  codecs: CodecInfo[]
): CompressionOptions {
  if (codecs.length === 0) {
    throw new Error("resolve requires at least one codec");
  }

  const formats = getAvailableFormats(codecs);
  let format: Format = partial.outputFormat ?? "mp4";
  if (!formats.includes(format)) {
    format = (formats[0] ?? "mp4") as Format;
  }

  const allowedForFormat = getCodecsForFormat(format, codecs).map(
    (c) => c.value
  );
  let codec: Codec = (partial.codec ?? allowedForFormat[0]) as Codec;
  const oldCodec = codec;
  if (!allowedForFormat.includes(codec)) {
    codec = (allowedForFormat[0] ?? "libx264") as Codec;
  }

  const codecInfo = getCodecInfo(codec, codecs);
  const supportsTune = codecInfo?.supportsTune ?? false;
  const tune = supportsTune ? (partial.tune ?? undefined) : undefined;

  const quality =
    oldCodec !== codec
      ? convertQualityForCodecSwitch(
          partial.quality ?? DEFAULT_OPTIONS.quality,
          oldCodec,
          codec
        )
      : (partial.quality ?? DEFAULT_OPTIONS.quality);

  return {
    ...DEFAULT_OPTIONS,
    ...partial,
    codec,
    outputFormat: format,
    tune,
    quality,
  };
}

export function createInitialOptions(
  codecs: CodecInfo[],
  presetId: BasicPresetId
): CompressionOptions {
  const overlay = BASIC_PRESETS[presetId];
  const format = overlay.outputFormat;
  const compatibleForFormat = getCodecsForFormat(format, codecs);
  const defaultCodec = getFormatCapabilities(format).defaultCodec;
  const initialCodec = compatibleForFormat.some((c) => c.value === defaultCodec)
    ? defaultCodec
    : compatibleForFormat[0]?.value ?? codecs[0].value;

  const base: Partial<CompressionOptions> = {
    codec: initialCodec as Codec,
    outputFormat: format,
    preset: "fast",
    fps: 30,
    scale: 1,
    removeAudio: false,
    generatePreview: true,
    previewDuration: 3,
    tune: undefined,
  };

  const codecInfo = getCodecInfo(initialCodec, codecs);
  if (!codecInfo?.supportsTune) {
    base.tune = undefined;
  }

  return resolve({ ...base, ...overlay }, codecs);
}

export function applyPreset(
  current: CompressionOptions,
  presetId: BasicPresetId,
  codecs: CodecInfo[]
): CompressionOptions {
  const overlay = BASIC_PRESETS[presetId];
  return resolve({ ...current, ...overlay }, codecs);
}
