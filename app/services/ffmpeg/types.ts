export type PresetOptions =
  | 'ultrafast'
  | 'superfast'
  | 'veryfast'
  | 'faster'
  | 'fast'
  | 'medium'
  | 'slow'
  | 'slower'
  | 'veryslow'

export type TranscodeOptions = {
  codec?: string
  quality: number
  bitrate?: number
  format?: string
  scale: number
  preset: PresetOptions
  fps: number
  removeAudio: boolean
  previewDuration?: number
}

export type TranscodeOutput = {
  file: Blob
  name: string
}

export type PreviewOutput = {
  original: Blob
  compressed: Blob
  estimatedSize: number
}

export const IPC_CHANNELS = {
  FFMPEG_CHECK_AVAILABILITY: 'ffmpeg:check-availability',
  FFMPEG_TRANSCODE: 'ffmpeg:transcode',
  FFMPEG_PREVIEW: 'ffmpeg:preview',
  FFMPEG_PROGRESS: 'ffmpeg:progress',
  FFMPEG_ERROR: 'ffmpeg:error',
  FFMPEG_COMPLETE: 'ffmpeg:complete',
  FFMPEG_TERMINATE: 'ffmpeg:terminate',
} as const

export const DEFAULTS = {
  PREVIEW_DURATION: 3,
  CODEC: 'libx264',
  QUALITY: 100,
  BITRATE: 2000,
  SCALE: 1,
  REMOVE_AUDIO: false,
  FPS: 30,
  PRESET: 'fast' as PresetOptions,
} as const

export interface FFmpegAPI {
  checkAvailability: () => Promise<boolean>
  transcode: (data: { file: ArrayBuffer; name: string; options: TranscodeOptions }) => Promise<TranscodeOutput>
  generatePreview: (data: { file: ArrayBuffer; name: string; options: TranscodeOptions }) => Promise<PreviewOutput>
  terminate: () => Promise<void>
  onProgress: (callback: (progress: number) => void) => () => void
  onError: (callback: (error: string) => void) => () => void
  onComplete: (callback: () => void) => () => void
}

declare global {
  interface Window {
    ffmpeg: FFmpegAPI
  }
}
