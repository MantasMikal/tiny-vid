import { IpcMainInvokeEvent } from 'electron'
import { readFile } from 'fs/promises'
import { TranscodeOptions, DEFAULTS } from '@/lib/ffmpeg/types'
import { getQuality, getCodecPreset } from './ffmpeg-utils'
import { FFmpegCommandBuilder } from './ffmpeg-builder'
import { FFmpegRunner } from './ffmpeg-runner'
import { TempFileManager } from './ffmpeg-temp'

export function buildFFmpegCommand(inputPath: string, outputPath: string, options: TranscodeOptions): string[] {
  const {
    codec = DEFAULTS.CODEC,
    quality = DEFAULTS.QUALITY,
    maxBitrate,
    scale = DEFAULTS.SCALE,
    fps = DEFAULTS.FPS,
    removeAudio = DEFAULTS.REMOVE_AUDIO,
    preset = DEFAULTS.PRESET,
    tune,
  } = options

  const crf = getQuality(quality, codec)
  const codecPreset = getCodecPreset(preset, codec)
  const builder = new FFmpegCommandBuilder(codec)
    .input(inputPath)
    .audio(!removeAudio)
    .scale(scale)
    .preset(codecPreset)
    .fps(fps)
    .fastStart()
    .tune(tune)

  if (maxBitrate) {
    builder.constrainedCrf(crf, maxBitrate)
  } else {
    builder.crf(crf)
  }

  return builder.output(outputPath).build()
}

export async function processVideo(
  inputPath: string,
  outputPath: string,
  options: TranscodeOptions,
  event: IpcMainInvokeEvent
): Promise<ArrayBuffer> {
  const runner = new FFmpegRunner()
  const command = buildFFmpegCommand(inputPath, outputPath, options)
  await runner.run(command, event)

  const outputBuffer = await readFile(outputPath)
  return new Uint8Array(outputBuffer).buffer
}

export async function transcodeVideo(
  event: IpcMainInvokeEvent,
  data: { file: ArrayBuffer; name: string; options: TranscodeOptions }
): Promise<{ file: ArrayBuffer; name: string } | null> {
  const tempFiles = new TempFileManager()

  try {
    const inputPath = await tempFiles.create('input.mp4', Buffer.from(data.file))
    const outputPath = await tempFiles.create('output.mp4')

    const outputBuffer = await processVideo(inputPath, outputPath, data.options, event)

    return {
      file: outputBuffer,
      name: `compressed-${data.name}`,
    }
  } catch (error) {
    if (error instanceof Error && error.name === 'AbortError') {
      // Process was manually terminated, return null to indicate cancellation
      return null
    }
    // Re-throw actual errors to be handled by caller
    throw error
  } finally {
    await tempFiles.cleanup()
  }
}

export async function generatePreview(
  event: IpcMainInvokeEvent,
  data: { file: ArrayBuffer; name: string; options: TranscodeOptions }
): Promise<{ original: ArrayBuffer; compressed: ArrayBuffer; estimatedSize: number } | null> {
  const tempFiles = new TempFileManager()

  try {
    const inputPath = await tempFiles.create('input.mp4', Buffer.from(data.file))
    const previewDuration = data.options.previewDuration ?? DEFAULTS.PREVIEW_DURATION
    const originalPath = await tempFiles.create('preview-original.mp4')
    const outputPath = await tempFiles.create('preview-output.mp4')

    const extractCommand = [
      '-threads', '0',
      '-progress', 'pipe:1',
      '-ss', '0',
      '-t', previewDuration.toString(),
      '-i', inputPath,
      '-c', 'copy',
      originalPath
    ]

    const runner = new FFmpegRunner()
    await runner.run(extractCommand)

    const compressedBuffer = await processVideo(originalPath, outputPath, data.options, event)

    const originalBuffer = await readFile(originalPath)
    const ratio = compressedBuffer.byteLength / originalBuffer.length
    const estimatedSize = Math.round(data.file.byteLength * ratio)

    return {
      original: new Uint8Array(originalBuffer).buffer,
      compressed: compressedBuffer,
      estimatedSize,
    }
  } catch (error) {
    if (error instanceof Error && error.name === 'AbortError') {
      // Process was manually terminated, return null to indicate cancellation
      return null
    }
    // Re-throw actual errors to be handled by caller
    throw error
  } finally {
    await tempFiles.cleanup()
  }
} 