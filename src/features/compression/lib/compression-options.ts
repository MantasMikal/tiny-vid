import type { CodecInfo } from "@/types/tauri";

export type LicenseProfile = "standalone" | "lgpl";

/**
 * Codec metadata. Must stay in sync with backend src-tauri/src/codec.rs CODEC_TABLE.
 * presetType "vt" = VideoToolbox (hardware); others = software encoders.
 */
const CODEC_REGISTRY = {
  libx264: {
    name: "H.264 (Widest support)",
    supportsTune: true,
    presetType: "x264",
    formats: ["mp4", "mkv"],
  },
  libx265: {
    name: "H.265 (Smaller files)",
    supportsTune: false,
    presetType: "x265",
    formats: ["mp4", "mkv"],
  },
  libsvtav1: {
    name: "AV1 (Smallest files)",
    supportsTune: false,
    presetType: "av1",
    formats: ["mp4", "webm", "mkv"],
  },
  "libvpx-vp9": {
    name: "VP9 (Browser-friendly WebM)",
    supportsTune: false,
    presetType: "vp9",
    formats: ["webm", "mkv"],
  },
  h264_videotoolbox: {
    name: "H.264 (VideoToolbox)",
    supportsTune: false,
    presetType: "vt",
    formats: ["mp4", "mkv"],
  },
  hevc_videotoolbox: {
    name: "H.265 (VideoToolbox)",
    supportsTune: false,
    presetType: "vt",
    formats: ["mp4", "mkv"],
  },
} as const;

const FORMAT_REGISTRY = {
  mp4: {
    name: "MP4",
    extension: "mp4",
    codecs: ["libx264", "libx265", "libsvtav1", "h264_videotoolbox", "hevc_videotoolbox"],
    defaultCodec: "libx264",
  },
  webm: {
    name: "WebM",
    extension: "webm",
    codecs: ["libvpx-vp9", "libsvtav1"],
    defaultCodec: "libvpx-vp9",
  },
  mkv: {
    name: "MKV",
    extension: "mkv",
    codecs: [
      "libx264",
      "libx265",
      "libsvtav1",
      "libvpx-vp9",
      "h264_videotoolbox",
      "hevc_videotoolbox",
    ],
    defaultCodec: "libx264",
  },
} as const;

export type Codec = keyof typeof CODEC_REGISTRY;
export type Format = keyof typeof FORMAT_REGISTRY;

const CODECS = Object.keys(CODEC_REGISTRY) as Codec[];

const FORMATS: Format[] = ["mp4", "webm", "mkv"];

/** Validates against backend codecs when provided; otherwise against known codec set. */
export function isCodec(s: string, availableCodecs?: CodecInfo[]): s is Codec {
  if (availableCodecs?.length) {
    return availableCodecs.some((c) => c.value === s);
  }
  return (CODECS as readonly string[]).includes(s);
}

export function isFormat(s: string): s is Format {
  return (FORMATS as readonly string[]).includes(s);
}

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

export function getCompatibleFormats(codec: Codec): Format[] {
  return (Object.entries(FORMAT_REGISTRY) as [Format, (typeof FORMAT_REGISTRY)[Format]][])
    .filter(([, def]) => (def.codecs as readonly Codec[]).includes(codec))
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

export type PresetValue = (typeof presets)[number]["value"];

export function isPresetValue(s: string): s is PresetValue {
  return presets.some((p) => p.value === s);
}

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

export function getTuneOptionsForCodec(codec: Codec): readonly { name: string; value: string }[] {
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
  preserveAdditionalAudioStreams?: boolean;
}

function getCrfRange(codec: string): { low: number; high: number } {
  const c = codec.toLowerCase();
  if (c.includes("videotoolbox")) return { low: 0, high: 100 };
  if (c.includes("x265") || c.includes("hevc")) return { low: 28, high: 51 };
  if (c.includes("svtav1")) return { low: 24, high: 63 };
  if (c.includes("vp9") || c.includes("vpx")) return { low: 20, high: 63 };
  return { low: 23, high: 51 };
}

export function qualityToCrf(quality: number, codec: string): number {
  const c = codec.toLowerCase();
  if (c.includes("videotoolbox")) return Math.round(Math.min(quality, 100));
  const q = Math.min(quality, 100) / 100;
  const { low, high } = getCrfRange(codec);
  return Math.round(high - q * (high - low));
}

function crfToQuality(crf: number, codec: string): number {
  const c = codec.toLowerCase();
  if (c.includes("videotoolbox")) return Math.round(Math.max(0, Math.min(100, crf)));
  const { low, high } = getCrfRange(codec);
  const q = (high - crf) / (high - low);
  return Math.round(Math.max(0, Math.min(100, q * 100)));
}

function getPerceptualOffset(codec: string): number {
  const c = codec.toLowerCase();
  if (c.includes("videotoolbox")) return 0;
  if (c.includes("x265") || c.includes("hevc")) return 5;
  if (c.includes("svtav1")) return 12;
  if (c.includes("vp9") || c.includes("vpx")) return 8;
  return 0;
}

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

export function getCodecsForFormat(format: string, codecs: CodecInfo[]): CodecInfo[] {
  return codecs.filter((c) => c.formats.includes(format));
}

export function getAvailableFormats(codecs: CodecInfo[]): string[] {
  return [...new Set(codecs.flatMap((c) => c.formats))];
}

export function getCodecInfo(value: string, codecs: CodecInfo[]): CodecInfo | undefined {
  return codecs.find((c) => c.value === value);
}
