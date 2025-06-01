import { ipcMain, IpcMainInvokeEvent } from 'electron'
import { spawn, ChildProcess } from 'child_process'
import { join } from 'path'
import { writeFile, readFile, unlink } from 'fs/promises'
import { tmpdir } from 'os'
import { IPC_CHANNELS, TranscodeOptions, DEFAULTS } from '@/app/services/ffmpeg/types'
import { getFFmpegPath } from './platform'

function qualityToCrf(quality: number): number {
  const clampedQuality = Math.min(Math.max(quality, 0), 100)
  return Math.round(51 - (clampedQuality / 100) * (51 - 23))
}

// Replace the single process tracking with a Set of active processes
const activeFFmpegProcesses = new Set<ChildProcess>()

function parseFFmpegProgress(
  output: string,
  currentDuration: number | null
): { progress: number | null; duration: number | null } {
  // Get duration from stderr output
  const durationMatch = output.match(/Duration: (\d+):(\d+):(\d+.\d+)/)
  if (durationMatch) {
    const [, hours, minutes, seconds] = durationMatch
    const duration = parseFloat(hours) * 3600 + parseFloat(minutes) * 60 + parseFloat(seconds)
    return { progress: null, duration }
  }

  // Get time progress from the progress pipe format
  const timeMatch = output.match(/out_time_ms=(\d+)/)
  if (timeMatch && currentDuration !== null) {
    const currentTimeMs = parseInt(timeMatch[1], 10)
    const currentTime = currentTimeMs / 1000000 // Convert microseconds to seconds
    return { progress: Math.min(currentTime / currentDuration, 1), duration: currentDuration } // Ensure we don't exceed 100%
  }

  return { progress: null, duration: currentDuration }
}

function sendProgress(event: IpcMainInvokeEvent, progress: number) {
  event.sender.send(IPC_CHANNELS.FFMPEG_PROGRESS, progress)
}

function handleFFmpegError(event: IpcMainInvokeEvent, error: Error) {
  event.sender.send(IPC_CHANNELS.FFMPEG_ERROR, error.message)
}

function transcodeOptionsToArgs(options: TranscodeOptions): string[] {
  const {
    codec = DEFAULTS.CODEC,
    quality = DEFAULTS.QUALITY,
    scale = DEFAULTS.SCALE,
    fps = DEFAULTS.FPS,
    removeAudio = DEFAULTS.REMOVE_AUDIO,
    preset = DEFAULTS.PRESET,
  } = options

  const args = ['-progress', 'pipe:1']

  if (removeAudio) {
    args.push('-an')
  } else {
    args.push('-c:a', 'aac', '-b:a', '128k')
  }

  if (codec) {
    args.push('-c:v', codec)
  }

  if (quality !== undefined) {
    args.push('-crf', qualityToCrf(quality).toString())
  }

  if (scale && scale < 1) {
    args.push('-vf', `scale=round(iw*${scale}/2)*2:-2`)
  }

  if (preset) {
    args.push('-preset', preset)
  }

  if (fps) {
    args.push('-r', fps.toString())
  }

  return args
}

interface FFmpegExecOptions {
  inputPath: string
  outputPath: string
  options: TranscodeOptions
  event?: IpcMainInvokeEvent
  trackProgress?: boolean
  additionalArgs?: string[]
}

interface FFmpegResult<T> {
  data: T
  cleanup: () => Promise<void>
}

async function execFFmpeg<T>({
  inputPath,
  outputPath,
  options,
  event,
  trackProgress = false,
  additionalArgs = [],
}: FFmpegExecOptions): Promise<FFmpegResult<T>> {
  const args = ['-i', inputPath, ...transcodeOptionsToArgs(options), ...additionalArgs, outputPath]
  const process = spawn(await getFFmpegPath(), args)
  activeFFmpegProcesses.add(process)

  let stderrBuffer = ''
  let videoDuration: number | null = null

  if (trackProgress && event) {
    // Handle stdout for progress updates
    process.stdout.on('data', (chunk: Buffer) => {
      const { progress, duration } = parseFFmpegProgress(chunk.toString(), videoDuration)
      if (duration !== null) videoDuration = duration
      if (progress !== null) sendProgress(event, progress)
    })

    // Handle stderr for duration info and progress
    process.stderr.on('data', (chunk: Buffer) => {
      stderrBuffer += chunk.toString()
      const lines = stderrBuffer.split('\n')
      stderrBuffer = lines.pop() || ''

      for (const line of lines) {
        if (line.trim()) {
          const { progress, duration } = parseFFmpegProgress(line, videoDuration)
          if (duration !== null) videoDuration = duration
          if (progress !== null) sendProgress(event, progress)
        }
      }
    })
  }

  return new Promise((resolve, reject) => {
    process.on('error', (error: Error) => {
      activeFFmpegProcesses.delete(process)
      if (event) handleFFmpegError(event, error)
      reject(error)
    })

    process.on('close', async (code: number) => {
      activeFFmpegProcesses.delete(process)
      if (code !== 0) {
        const error = new Error(`FFmpeg process failed (code ${code})`)
        if (event) handleFFmpegError(event, error)
        reject(error)
        return
      }

      try {
        const outputBuffer = await readFile(outputPath)
        const arrayBuffer = new Uint8Array(outputBuffer).buffer

        if (event) {
          event.sender.send(IPC_CHANNELS.FFMPEG_COMPLETE)
        }

        resolve({
          data: arrayBuffer as T,
          cleanup: async () => {
            await Promise.all([unlink(inputPath).catch(() => {}), unlink(outputPath).catch(() => {})])
          },
        })
      } catch (error) {
        if (event) handleFFmpegError(event, error as Error)
        reject(error)
      }
    })
  })
}

async function handleFFmpegTranscode(
  event: IpcMainInvokeEvent,
  data: { file: ArrayBuffer; name: string; options: TranscodeOptions }
) {
  const tempDir = tmpdir()
  const inputPath = join(tempDir, `input-${Date.now()}.mp4`)
  const intermediatePath = join(tempDir, `intermediate-${Date.now()}.mp4`)
  const outputPath = join(tempDir, `output-${Date.now()}.mp4`)

  try {
    await writeFile(inputPath, Buffer.from(data.file))

    // First copy the file to ensure it's in a proper format
    const copyProcess = spawn(await getFFmpegPath(), ['-i', inputPath, '-c', 'copy', intermediatePath])
    await new Promise<void>((resolve, reject) => {
      copyProcess.on('error', reject)
      copyProcess.on('close', (code) => {
        if (code === 0) resolve()
        else reject(new Error(`Failed to prepare video for transcoding (code ${code})`))
      })
    })

    const { data: outputBuffer, cleanup } = await execFFmpeg<ArrayBuffer>({
      inputPath: intermediatePath,
      outputPath,
      options: data.options,
      event,
      trackProgress: true,
    })

    await cleanup()
    return { file: outputBuffer, name: `compressed-${data.name}` }
  } catch (error) {
    handleFFmpegError(event, error as Error)
    throw error
  }
}

async function handleFFmpegPreview(
  event: IpcMainInvokeEvent,
  data: { file: ArrayBuffer; name: string; options: TranscodeOptions }
) {
  const tempDir = tmpdir()
  const inputPath = join(tempDir, `preview-input-${Date.now()}.mp4`)
  const originalPath = join(tempDir, `preview-original-${Date.now()}.mp4`)
  const outputPath = join(tempDir, `preview-output-${Date.now()}.mp4`)

  try {
    await writeFile(inputPath, Buffer.from(data.file))
    const previewDuration = data.options.previewDuration ?? DEFAULTS.PREVIEW_DURATION

    // Extract preview segment
    const extractProcess = spawn(await getFFmpegPath(), [
      '-ss',
      '0',
      '-i',
      inputPath,
      '-t',
      previewDuration.toString(),
      '-c',
      'copy',
      originalPath,
    ])

    await new Promise<void>((resolve, reject) => {
      extractProcess.on('error', reject)
      extractProcess.on('close', (code) => {
        if (code === 0) resolve()
        else reject(new Error(`Failed to extract video preview (code ${code})`))
      })
    })

    const { data: compressedBuffer, cleanup } = await execFFmpeg<ArrayBuffer>({
      inputPath: originalPath,
      outputPath,
      options: data.options,
      event,
      trackProgress: true,
    })

    const originalBuffer = await readFile(originalPath)
    const ratio = compressedBuffer.byteLength / originalBuffer.length
    const estimatedSize = Math.round(data.file.byteLength * ratio)

    await cleanup()
    return {
      original: new Uint8Array(originalBuffer).buffer,
      compressed: compressedBuffer,
      estimatedSize,
    }
  } catch (error) {
    handleFFmpegError(event, error as Error)
    throw error
  }
}

export function setupFFmpegHandlers() {
  ipcMain.handle(IPC_CHANNELS.FFMPEG_CHECK_AVAILABILITY, async () => {
    try {
      const ffmpegPath = await getFFmpegPath()
      const ffmpeg = spawn(ffmpegPath, ['-version'])
      return new Promise<boolean>((resolve) => {
        ffmpeg.on('close', (code) => resolve(code === 0))
        ffmpeg.on('error', () => resolve(false))
      })
    } catch {
      return false
    }
  })

  ipcMain.handle(IPC_CHANNELS.FFMPEG_TRANSCODE, handleFFmpegTranscode)
  ipcMain.handle(IPC_CHANNELS.FFMPEG_PREVIEW, handleFFmpegPreview)

  ipcMain.handle(IPC_CHANNELS.FFMPEG_TERMINATE, () => {
    // Terminate all active processes
    for (const process of activeFFmpegProcesses) {
      process.kill()
    }
    activeFFmpegProcesses.clear()
  })
}
