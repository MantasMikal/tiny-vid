import { contextBridge, ipcRenderer } from 'electron';
import { IPC_CHANNELS } from '@/lib/ffmpeg/types';

const createListener = (channel: string, callback: (...args: any[]) => void) => {
  const listener = (_: any, ...args: any[]) => callback(...args);
  ipcRenderer.on(channel, listener);
  return () => ipcRenderer.removeListener(channel, listener);
};

contextBridge.exposeInMainWorld('ffmpeg', {
  transcode: (data: { file: ArrayBuffer; name: string; options: any }) =>
    ipcRenderer.invoke(IPC_CHANNELS.FFMPEG_TRANSCODE, data),
  generatePreview: (data: { file: ArrayBuffer; name: string; options: any }) =>
    ipcRenderer.invoke(IPC_CHANNELS.FFMPEG_PREVIEW, data),
  terminate: () => ipcRenderer.invoke(IPC_CHANNELS.FFMPEG_TERMINATE),
  onProgress: (callback: (progress: number) => void) =>
    createListener(IPC_CHANNELS.FFMPEG_PROGRESS, callback),
  onError: (callback: (error: string) => void) =>
    createListener(IPC_CHANNELS.FFMPEG_ERROR, callback),
  onComplete: (callback: () => void) =>
    createListener(IPC_CHANNELS.FFMPEG_COMPLETE, callback),
}); 