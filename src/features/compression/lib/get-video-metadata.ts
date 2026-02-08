import { desktopClient } from "@/platform/desktop/client";
import type { GetVideoMetadataResult } from "@/types/native";

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
  audioStreamCount: number;
  subtitleStreamCount?: number;
  audioCodecName?: string;
  audioChannels?: number;
  encoder?: string;
}

export async function getVideoMetadataFromPath(filePath: string): Promise<VideoMetadata> {
  const inspectResult = await desktopClient.invoke("media.inspect", {
    kind: "metadata",
    inputPath: filePath,
  });

  if (typeof inspectResult === "string") {
    throw new Error("Invalid metadata response from sidecar");
  }

  const meta: GetVideoMetadataResult = inspectResult;
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
    audioStreamCount: meta.audioStreamCount,
    subtitleStreamCount: meta.subtitleStreamCount,
    audioCodecName: meta.audioCodecName,
    audioChannels: meta.audioChannels,
    encoder: meta.encoder,
  };
}
