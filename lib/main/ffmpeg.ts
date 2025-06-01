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

const activeFFmpegProcesses = new Set<ChildProcess>()

function parseFFmpegProgress(
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

function sendProgress(event: IpcMainInvokeEvent, progress: number) {
  event.sender.send(IPC_CHANNELS.FFMPEG_PROGRESS, progress)
}

function handleFFmpegError(event: IpcMainInvokeEvent, error: Error) {
  event.sender.send(IPC_CHANNELS.FFMPEG_ERROR, error.message)
}

class TempFileManager {
  private files: string[] = []
  
  async create(suffix: string, content?: Buffer): Promise<string> {
    const path = join(tmpdir(), `ffmpeg-${Date.now()}-${Math.random().toString(36).substr(2, 9)}-${suffix}`)
    this.files.push(path)
    if (content) {
      await writeFile(path, content)
    }
    return path
  }
  
  async cleanup(): Promise<void> {
    await Promise.allSettled(this.files.map(f => unlink(f).catch(() => {})))
    this.files = []
  }
}

class FFmpegCommandBuilder {
  private args: string[] = ['-progress', 'pipe:1']
  
  input(path: string): this {
    this.args.push('-i', path)
    return this
  }
  
  output(path: string): this {
    this.args.push(path)
    return this
  }
  
  codec(codec: string): this {
    this.args.push('-c:v', codec)
    return this
  }
  
  crf(crf: number): this {
    this.args.push('-crf', crf.toString())
    return this
  }
  
  constrainedCrf(crf: number, maxBitrate: number): this {
    this.args.push('-crf', crf.toString(), '-maxrate', `${maxBitrate}k`, '-bufsize', `${maxBitrate * 2}k`)
    return this
  }
  
  audio(enabled: boolean): this {
    if (enabled) {
      this.args.push('-c:a', 'aac', '-b:a', '128k')
    } else {
      this.args.push('-an')
    }
    return this
  }
  
  scale(factor: number): this {
    if (factor < 1) {
      this.args.push('-vf', `scale=round(iw*${factor}/2)*2:-2`)
    }
    return this
  }
  
  preset(preset: string): this {
    this.args.push('-preset', preset)
    return this
  }
  
  fps(fps: number): this {
    this.args.push('-r', fps.toString())
    return this
  }
  
  extractSegment(start: number, duration: number): this {
    this.args.push('-ss', start.toString(), '-t', duration.toString(), '-c', 'copy')
    return this
  }
  
  build(): string[] {
    return this.args
  }
}

class FFmpegRunner {
  async run(
    command: string[], 
    event?: IpcMainInvokeEvent,
    onProgress?: (progress: number) => void
  ): Promise<void> {
    const process = spawn(await getFFmpegPath(), command)
    activeFFmpegProcesses.add(process)
    
    let stderrBuffer = ''
    let duration: number | null = null
    
    if (event || onProgress) {
      process.stdout.on('data', (chunk: Buffer) => {
        const { progress, duration: d } = parseFFmpegProgress(chunk.toString(), duration)
        if (d !== null) duration = d
        if (progress !== null) {
          if (onProgress) onProgress(progress)
          if (event) sendProgress(event, progress)
        }
      })

      process.stderr.on('data', (chunk: Buffer) => {
        stderrBuffer += chunk.toString()
        const lines = stderrBuffer.split('\n')
        stderrBuffer = lines.pop() || ''

        for (const line of lines) {
          if (line.trim()) {
            const { progress, duration: d } = parseFFmpegProgress(line, duration)
            if (d !== null) duration = d
            if (progress !== null) {
              if (onProgress) onProgress(progress)
              if (event) sendProgress(event, progress)
            }
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
      
      process.on('close', (code: number) => {
        activeFFmpegProcesses.delete(process)
        
        if (code === 0) {
          resolve()
        } else {
          const error = new Error(`FFmpeg failed (code ${code}): ${stderrBuffer}`)
          if (event) handleFFmpegError(event, error)
          reject(error)
        }
      })
    })
  }
}

function buildFFmpegCommand(
  inputPath: string, 
  outputPath: string, 
  options: TranscodeOptions
): string[] {
  const {
    codec = DEFAULTS.CODEC,
    quality = DEFAULTS.QUALITY,
    maxBitrate,
    scale = DEFAULTS.SCALE,
    fps = DEFAULTS.FPS,
    removeAudio = DEFAULTS.REMOVE_AUDIO,
    preset = DEFAULTS.PRESET,
  } = options

  const crf = qualityToCrf(quality)
  const builder = new FFmpegCommandBuilder()
    .input(inputPath)
    .codec(codec)
    .audio(!removeAudio)
    .scale(scale)
    .preset(preset)
    .fps(fps)

  if (maxBitrate) {
    builder.constrainedCrf(crf, maxBitrate)
  } else {
    builder.crf(crf)
  }

  return builder.output(outputPath).build()
}

async function processVideo(
  inputPath: string,
  outputPath: string,
  options: TranscodeOptions,
  event: IpcMainInvokeEvent
): Promise<ArrayBuffer> {
  const runner = new FFmpegRunner()
  const tempFiles = new TempFileManager()

  try {
    const command = buildFFmpegCommand(inputPath, outputPath, options)
    await runner.run(command, event)

    const outputBuffer = await readFile(outputPath)
    return new Uint8Array(outputBuffer).buffer
  } finally {
    await tempFiles.cleanup()
  }
}

async function transcodeVideo(
  event: IpcMainInvokeEvent,
  data: { file: ArrayBuffer; name: string; options: TranscodeOptions }
): Promise<{ file: ArrayBuffer; name: string }> {
  const tempFiles = new TempFileManager()
  
  try {
    const inputPath = await tempFiles.create('input.mp4', Buffer.from(data.file))
    const outputPath = await tempFiles.create('output.mp4')
    
    const outputBuffer = await processVideo(inputPath, outputPath, data.options, event)
    
    event.sender.send(IPC_CHANNELS.FFMPEG_COMPLETE)
    
    return {
      file: outputBuffer,
      name: `compressed-${data.name}`,
    }
  } catch (error) {
    handleFFmpegError(event, error as Error)
    throw error
  } finally {
    await tempFiles.cleanup()
  }
}

async function generatePreview(
  event: IpcMainInvokeEvent,
  data: { file: ArrayBuffer; name: string; options: TranscodeOptions }
): Promise<{ original: ArrayBuffer; compressed: ArrayBuffer; estimatedSize: number }> {
  const tempFiles = new TempFileManager()
  
  try {
    const inputPath = await tempFiles.create('input.mp4', Buffer.from(data.file))
    const previewDuration = data.options.previewDuration ?? DEFAULTS.PREVIEW_DURATION
    const originalPath = await tempFiles.create('preview-original.mp4')
    const outputPath = await tempFiles.create('preview-output.mp4')
    
    const extractCommand = new FFmpegCommandBuilder()
      .input(inputPath)
      .extractSegment(0, previewDuration)
      .output(originalPath)
      .build()
    
    const runner = new FFmpegRunner()
    await runner.run(extractCommand)
    
    const compressedBuffer = await processVideo(originalPath, outputPath, data.options, event)
    
    const originalBuffer = await readFile(originalPath)
    const ratio = compressedBuffer.byteLength / originalBuffer.length
    const estimatedSize = Math.round(data.file.byteLength * ratio)
    
    event.sender.send(IPC_CHANNELS.FFMPEG_COMPLETE)
    
    return {
      original: new Uint8Array(originalBuffer).buffer,
      compressed: compressedBuffer,
      estimatedSize,
    }
  } catch (error) {
    handleFFmpegError(event, error as Error)
    throw error
  } finally {
    await tempFiles.cleanup()
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

  ipcMain.handle(IPC_CHANNELS.FFMPEG_TRANSCODE, (event, data) =>
    transcodeVideo(event, data)
  )
  
  ipcMain.handle(IPC_CHANNELS.FFMPEG_PREVIEW, (event, data) =>
    generatePreview(event, data)
  )

  ipcMain.handle(IPC_CHANNELS.FFMPEG_TERMINATE, () => {
    for (const process of activeFFmpegProcesses) {
      process.kill()
    }
    activeFFmpegProcesses.clear()
  })
}
