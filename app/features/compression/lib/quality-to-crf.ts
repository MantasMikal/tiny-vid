/**
 * Converts a slider value (0-100) to a CRF value (23-51) for ffmpeg.
 * @param {number} quality - The quality value from the slider (0-100).
 * @returns {number} - The corresponding CRF value (23-51).
 */
export const qualityToCrf = (quality: number): number => {
  // Ensure quality is within 0-100
  const clampedQuality = Math.min(Math.max(quality, 0), 100);

  // Map the slider value to a CRF value
  // Quality 0 maps to CRF 51, Quality 100 maps to CRF 23
  const crf = 51 - (clampedQuality / 100) * (51 - 23);

  // Round to the nearest integer
  return Math.round(crf);
};
