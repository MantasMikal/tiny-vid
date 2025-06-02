import { spawn, ChildProcess } from 'child_process'
import { IpcMainInvokeEvent } from 'electron'
import { getFFmpegPath } from '../main/platform'
import { parseFFmpegProgress } from './ffmpeg-progress'
import { IPC_CHANNELS } from '@/lib/ffmpeg/types'

const activeFFmpegProcesses = new Set<ChildProcess>()
const terminatedProcesses = new Set<ChildProcess>()

function sendProgress(event: IpcMainInvokeEvent, progress: number) {
  event.sender.send(IPC_CHANNELS.FFMPEG_PROGRESS, progress)
}

function handleFFmpegError(event: IpcMainInvokeEvent, error: Error) {
  event.sender.send(IPC_CHANNELS.FFMPEG_ERROR, error.message)
}

export class FFmpegRunner {
  async run(command: string[], event?: IpcMainInvokeEvent, onProgress?: (progress: number) => void): Promise<void> {
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
        terminatedProcesses.delete(process)
        if (event) handleFFmpegError(event, error)
        reject(error)
      })

      process.on('close', (code: number) => {
        const wasTerminated = terminatedProcesses.has(process)
        activeFFmpegProcesses.delete(process)
        terminatedProcesses.delete(process)

        if (code === 0) {
          resolve()
        } else if (wasTerminated) {
          // Process was manually terminated, create a special abort error
          const abortError = new Error('Process was terminated')
          abortError.name = 'AbortError'
          reject(abortError)
        } else {
          const error = new Error(`FFmpeg failed (code ${code}): ${stderrBuffer}`)
          if (event) handleFFmpegError(event, error)
          reject(error)
        }
      })
    })
  }
}

export function terminateAllFFmpegProcesses(): void {
  for (const process of activeFFmpegProcesses) {
    terminatedProcesses.add(process)
    process.kill()
  }
  activeFFmpegProcesses.clear()
} 