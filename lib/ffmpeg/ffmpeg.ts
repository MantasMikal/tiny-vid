import { ipcMain, IpcMainInvokeEvent } from 'electron'
import { IPC_CHANNELS } from '@/app/services/ffmpeg/types'
import { terminateAllFFmpegProcesses } from './ffmpeg-runner'
import { transcodeVideo, generatePreview } from './ffmpeg-commands'

function handleFFmpegError(event: IpcMainInvokeEvent, error: Error) {
  event.sender.send(IPC_CHANNELS.FFMPEG_ERROR, error.message)
}

async function handleTranscodeVideo(event: IpcMainInvokeEvent, data: any) {
  try {
    const result = await transcodeVideo(event, data)
    if (result) event.sender.send(IPC_CHANNELS.FFMPEG_COMPLETE)
    return result
  } catch (error) {
    handleFFmpegError(event, error as Error)
    throw error
  }
}

async function handleGeneratePreview(event: IpcMainInvokeEvent, data: any) {
  try {
    const result = await generatePreview(event, data)
    if (result) event.sender.send(IPC_CHANNELS.FFMPEG_COMPLETE)
    return result
  } catch (error) {
    handleFFmpegError(event, error as Error)
    throw error
  }
}

export function setupFFmpegHandlers() {
  ipcMain.handle(IPC_CHANNELS.FFMPEG_TRANSCODE, handleTranscodeVideo)
  ipcMain.handle(IPC_CHANNELS.FFMPEG_PREVIEW, handleGeneratePreview)
  ipcMain.handle(IPC_CHANNELS.FFMPEG_TERMINATE, () => {
    terminateAllFFmpegProcesses()
  })
}
