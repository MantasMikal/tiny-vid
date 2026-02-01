export interface CompressionOptions {
  quality: number;
  maxBitrate?: number;
  preset: (typeof presets)[number]["value"];
  fps: number;
  scale: number;
  removeAudio: boolean;
  codec: string;
  generatePreview?: boolean;
  previewDuration?: number;
  tune?: string;
}

/** CRF ranges per codec - must match src-tauri/src/ffmpeg/builder.rs */
function getCrfRange(codec: string): { low: number; high: number } {
  const c = codec.toLowerCase();
  if (c.includes("x265") || c.includes("hevc")) return { low: 28, high: 51 };
  if (c.includes("svtav1")) return { low: 24, high: 63 };
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
  return 0;
}

/** When switching codec, convert quality so perceived result stays similar. */
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

export const codecs = [
  { name: "H.264 (Widest support)", value: "libx264" },
  { name: "H.265 (Smaller files)", value: "libx265" },
  { name: "AV1 (Smallest files)", value: "libsvtav1" },
];

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
];

export const maxBitratePresets = [
  { name: "No limit", value: "none" },
  { name: "Low (500 kbps)", value: 500 },
  { name: "Medium (1000 kbps)", value: 1000 },
  { name: "High (2000 kbps)", value: 2000 },
  { name: "Very High (4000 kbps)", value: 4000 },
  { name: "Ultra (8000 kbps)", value: 8000 },
  { name: "Custom", value: "custom" },
];

export const presets = [
  { name: "Ultra Fast", value: "ultrafast" },
  { name: "Super Fast", value: "superfast" },
  { name: "Very Fast", value: "veryfast" },
  { name: "Faster", value: "faster" },
  { name: "Fast", value: "fast" },
  { name: "Medium", value: "medium" },
  { name: "Slow", value: "slow" },
];
