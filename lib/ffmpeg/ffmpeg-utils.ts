/**
 * Clamp a 0–100 slider, then linearly map to [highCRF..lowCRF].
 *   - highCRF = the "best" (lowest) CRF for quality=100,
 *   - lowCRF  = the "worst" (highest) CRF for quality=0.
 */
function mapLinearCrf(quality: number, highCRF: number, lowCRF: number): number {
  const q = Math.min(Math.max(quality, 0), 100)
  // when q=100 → return highCRF; when q=0 → return lowCRF; linearly in between
  return Math.round(lowCRF - (q / 100) * (lowCRF - highCRF))
}

//–– 1) x264 (libx264): keep [23..51] ––//
function qualityToCrf(quality: number): number {
  const HIGH = 23 // "CRF 23" ≃ visually lossless
  const LOW = 51 // worst-quality boundary
  return mapLinearCrf(quality, HIGH, LOW)
}

//–– 2) x265 (libx265): keep [28..51] ––//
function qualityToHevcCrf(quality: number): number {
  const HIGH = 28 // "CRF 28" ≃ x264 CRF 23
  const LOW = 51 // worst-quality boundary
  return mapLinearCrf(quality, HIGH, LOW)
}

//–– 3) AV1 / libaom-av1: tweak to [24..63] instead of [20..63] ––//
//    (You can also pick 25 if you want AV1 to feel slightly "richer.")
function qualityToAv1Crf(quality: number): number {
  const HIGH = 24 // maps quality=100 → CRF 24 (closer to x265 28 ≃ x264 23)
  const LOW = 63 // worst-quality boundary
  return mapLinearCrf(quality, HIGH, LOW)
}

export const getQuality = (quality: number, codec: string): number => {
  if (codec === 'libx265' || codec === 'hevc') {
    return qualityToHevcCrf(quality)
  }
  if (codec === 'libaom-av1') {
    return qualityToAv1Crf(quality)
  }
  // Default → libx264
  return qualityToCrf(quality)
}

export function getCodecPreset(preset: string, codec: string): string {
  // AV1 doesn't use the same presets as x264/x265
  if (codec === 'libaom-av1') {
    const presetMap: Record<string, string> = {
      ultrafast: '8',
      superfast: '7',
      veryfast: '6',
      faster: '5',
      fast: '4',
      medium: '3',
      slow: '2',
    }
    return presetMap[preset] || '4'
  }
  return preset
}
