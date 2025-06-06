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
  codec: string
  quality: number
  maxBitrate?: number
  format?: string
  scale: number
  preset: PresetOptions
  fps: number
  removeAudio: boolean
  previewDuration?: number
  tune?: string
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
  FFMPEG_TRANSCODE: 'ffmpeg:transcode',
  FFMPEG_PREVIEW: 'ffmpeg:preview',
  FFMPEG_TERMINATE: 'ffmpeg:terminate',
  FFMPEG_PROGRESS: 'ffmpeg:progress',
  FFMPEG_ERROR: 'ffmpeg:error',
  FFMPEG_COMPLETE: 'ffmpeg:complete',
} as const

export const DEFAULTS = {
  PREVIEW_DURATION: 3,
  CODEC: 'libx264',
  QUALITY: 75,
  SCALE: 1,
  REMOVE_AUDIO: false,
  FPS: 30,
  PRESET: 'fast' as PresetOptions,
  TUNE: undefined,
} as const

export interface FFmpegAPI {
  transcode: (data: { file: ArrayBuffer; name: string; options: TranscodeOptions }) => Promise<{ file: ArrayBuffer; name: string } | null>
  generatePreview: (data: { file: ArrayBuffer; name: string; options: TranscodeOptions }) => Promise<{ original: ArrayBuffer; compressed: ArrayBuffer; estimatedSize: number } | null>
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
