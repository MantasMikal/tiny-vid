export function parseFFmpegProgress(
  output: string,
  currentDuration: number | null
): { progress: number | null; duration: number | null } {
  const durationMatch = output.match(/Duration: (\d+):(\d+):(\d+.\d+)/)
  if (durationMatch) {
    const [, hours, minutes, seconds] = durationMatch
    const duration = parseFloat(hours) * 3600 + parseFloat(minutes) * 60 + parseFloat(seconds)
    return { progress: null, duration }
  }

  const timeMatch = output.match(/out_time_ms=(\d+)/)
  if (timeMatch && currentDuration !== null) {
    const currentTimeMs = parseInt(timeMatch[1], 10)
    const currentTime = currentTimeMs / 1000000
    return { progress: Math.min(currentTime / currentDuration, 1), duration: currentDuration }
  }

  return { progress: null, duration: currentDuration }
} 