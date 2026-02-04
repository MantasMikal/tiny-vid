import { invoke } from "@tauri-apps/api/core";

import type { GetVideoMetadataResult } from "@/types/tauri";

export interface VideoMetadata {
  duration: number;
  width: number;
  height: number;
  size: number;
  sizeMB: number;
  fps: number;
  codecName?: string;
  codecLongName?: string;
  videoBitRate?: number;
  formatBitRate?: number;
  formatName?: string;
  formatLongName?: string;
  nbStreams?: number;
}

export async function getVideoMetadataFromPath(filePath: string): Promise<VideoMetadata> {
  const meta = await invoke<GetVideoMetadataResult>("get_video_metadata", {
    path: filePath,
  });
  return {
    duration: meta.duration,
    width: meta.width,
    height: meta.height,
    size: meta.size,
    sizeMB: meta.sizeMb,
    fps: meta.fps,
    codecName: meta.codecName,
    codecLongName: meta.codecLongName,
    videoBitRate: meta.videoBitRate,
    formatBitRate: meta.formatBitRate,
    formatName: meta.formatName,
    formatLongName: meta.formatLongName,
    nbStreams: meta.nbStreams,
  };
}
