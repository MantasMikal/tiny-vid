/** Codec registry: single source of truth. Types flow from keys. */
const CODEC_REGISTRY = {
  libx264: {
    name: "H.264 (Widest support)",
    supportsTune: true,
    presetType: "x264",
    formats: ["mp4"],
  },
  libx265: {
    name: "H.265 (Smaller files)",
    supportsTune: false,
    presetType: "x265",
    formats: ["mp4"],
  },
  libsvtav1: {
    name: "AV1 (Smallest files)",
    supportsTune: false,
    presetType: "av1",
    formats: ["mp4", "webm"],
  },
  "libvpx-vp9": {
    name: "VP9 (Browser-friendly WebM)",
    supportsTune: false,
    presetType: "vp9",
    formats: ["webm"],  
  },
} as const;

/** Format registry: single source of truth. Types flow from keys. */
const FORMAT_REGISTRY = {
  mp4: {
    name: "MP4",
    extension: "mp4",
    codecs: ["libx264", "libx265", "libsvtav1"],
    defaultCodec: "libx264",
  },
  webm: {
    name: "WebM",
    extension: "webm",
    codecs: ["libvpx-vp9", "libsvtav1"],
    defaultCodec: "libvpx-vp9",
  },
} as const;

export type Codec = keyof typeof CODEC_REGISTRY;
export type Format = keyof typeof FORMAT_REGISTRY;

export const codecs = (
  Object.entries(CODEC_REGISTRY) as [Codec, (typeof CODEC_REGISTRY)[Codec]][]
).map(([value, def]) => ({ name: def.name, value }));

export const outputFormats = (
  Object.entries(FORMAT_REGISTRY) as [Format, (typeof FORMAT_REGISTRY)[Format]][]
).map(([value, def]) => ({ name: def.name, value, extension: def.extension }));

export function getCodecCapabilities(codec: Codec) {
  return CODEC_REGISTRY[codec];
}

export function getFormatCapabilities(format: Format) {
  return FORMAT_REGISTRY[format];
}

export function getCompatibleCodecs(format: Format): Codec[] {
  return [...FORMAT_REGISTRY[format].codecs] as Codec[];
}

export function getCompatibleFormats(codec: Codec): Format[] {
  return (
    Object.entries(FORMAT_REGISTRY) as [Format, (typeof FORMAT_REGISTRY)[Format]][]
  )
    .filter(([, def]) =>
      (def.codecs as readonly Codec[]).includes(codec)
    )
    .map(([f]) => f);
}

export function getDefaultExtension(format: Format): string {
  return FORMAT_REGISTRY[format].extension;
}

export const presets = [
  { name: "Ultra Fast", value: "ultrafast" },
  { name: "Super Fast", value: "superfast" },
  { name: "Very Fast", value: "veryfast" },
  { name: "Faster", value: "faster" },
  { name: "Fast", value: "fast" },
  { name: "Medium", value: "medium" },
  { name: "Slow", value: "slow" },
] as const;

export const tuneOptions = [
  { name: "None (Default)", value: "none" },
  { name: "Film", value: "film" },
  { name: "Animation", value: "animation" },
  { name: "Grain", value: "grain" },
  { name: "Still Image", value: "stillimage" },
  { name: "Fast Decode", value: "fastdecode" },
  { name: "Zero Latency", value: "zerolatency" },
  { name: "PSNR", value: "psnr" },
  { name: "SSIM", value: "ssim" },
] as const;

export function getTuneOptionsForCodec(
  codec: Codec
): readonly { name: string; value: string }[] {
  return CODEC_REGISTRY[codec].supportsTune ? tuneOptions : [];
}

export const maxBitratePresets = [
  { name: "No limit", value: "none" },
  { name: "Low (500 kbps)", value: 500 },
  { name: "Medium (1000 kbps)", value: 1000 },
  { name: "High (2000 kbps)", value: 2000 },
  { name: "Very High (4000 kbps)", value: 4000 },
  { name: "Ultra (8000 kbps)", value: 8000 },
  { name: "Custom", value: "custom" },
] as const;

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
} as const;

export interface CompressionOptions {
  quality: number;
  maxBitrate?: number;
  preset: (typeof presets)[number]["value"];
  fps: number;
  scale: number;
  removeAudio: boolean;
  codec: Codec;
  outputFormat: Format;
  generatePreview?: boolean;
  previewDuration?: number;
  tune?: string;
}

/** Resolves partial options into valid CompressionOptions. Handles codec/format conflicts. */
export function resolveOptions(
  partial: Partial<CompressionOptions>
): CompressionOptions {
  const format: Format = partial.outputFormat ?? "mp4";
  const formatDef = FORMAT_REGISTRY[format];
  let codec = partial.codec ?? undefined;
  const oldCodec = codec;
  if (!codec || !(formatDef.codecs as readonly Codec[]).includes(codec)) {
    codec = formatDef.defaultCodec as Codec;
  }
  const codecDef = CODEC_REGISTRY[codec];
  const tune = codecDef.supportsTune ? partial.tune ?? undefined : undefined;

  const quality =
    oldCodec && oldCodec !== codec
      ? convertQualityForCodecSwitch(
          partial.quality ?? DEFAULT_OPTIONS.quality,
          oldCodec,
          codec
        )
      : partial.quality ?? DEFAULT_OPTIONS.quality;

  return {
    ...DEFAULT_OPTIONS,
    ...partial,
    codec,
    outputFormat: format,
    tune,
    quality,
  };
}

/** CRF ranges per codec - must match src-tauri/src/ffmpeg/builder.rs */
function getCrfRange(codec: string): { low: number; high: number } {
  const c = codec.toLowerCase();
  if (c.includes("x265") || c.includes("hevc")) return { low: 28, high: 51 };
  if (c.includes("svtav1")) return { low: 24, high: 63 };
  if (c.includes("vp9") || c.includes("vpx")) return { low: 20, high: 63 };
  return { low: 23, high: 51 };
}

/** Linear map quality 0-100 to CRF for the given codec. Mirrors Rust get_quality. */
export function qualityToCrf(quality: number, codec: string): number {
  const q = Math.min(quality, 100) / 100;
  const { low, high } = getCrfRange(codec);
  return Math.round(high - q * (high - low));
}

/** Inverse: CRF to quality 0-100 for the given codec. */
function crfToQuality(crf: number, codec: string): number {
  const { low, high } = getCrfRange(codec);
  const q = (high - crf) / (high - low);
  return Math.round(Math.max(0, Math.min(100, q * 100)));
}

/** Perceptual offset vs H.264 (used only when switching codec to preserve perceived quality). */
function getPerceptualOffset(codec: string): number {
  const c = codec.toLowerCase();
  if (c.includes("x265") || c.includes("hevc")) return 5;
  if (c.includes("svtav1")) return 12;
  if (c.includes("vp9") || c.includes("vpx")) return 8;
  return 0;
}

/** When switching codec, convert quality so perceived result stays similar. Uses getCrfRange per codec (VP9: 20–63, etc.). */
export function convertQualityForCodecSwitch(
  oldQuality: number,
  oldCodec: string,
  newCodec: string
): number {
  if (oldCodec === newCodec) return oldQuality;
  const effectiveCrfOld = qualityToCrf(oldQuality, oldCodec);
  const perceptualRef = effectiveCrfOld - getPerceptualOffset(oldCodec);
  const effectiveCrfNew = perceptualRef + getPerceptualOffset(newCodec);
  return crfToQuality(effectiveCrfNew, newCodec);
}
