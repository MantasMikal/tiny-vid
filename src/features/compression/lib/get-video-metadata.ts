import { convertFileSrc, invoke } from "@tauri-apps/api/core";

export interface VideoMetadata {
  duration: number;
  width: number;
  height: number;
  size: number;
  sizeMB: number;
}

export async function getVideoMetadata(
  filePath: string,
  size: number
): Promise<VideoMetadata> {
  return new Promise((resolve, reject) => {
    const video = document.createElement("video");
    video.preload = "metadata";
    const src = convertFileSrc(filePath);

    video.onloadedmetadata = () => {
      resolve({
        duration: video.duration,
        width: video.videoWidth,
        height: video.videoHeight,
        size,
        sizeMB: size / 1024 / 1024,
      });
    };

    video.onerror = () => {
      reject(new Error("Error loading video"));
    };

    video.src = src;
  });
}

export async function getVideoMetadataFromPath(
  filePath: string
): Promise<VideoMetadata> {
  const size = await invoke<number>("get_file_size", { path: filePath });
  return getVideoMetadata(filePath, size);
}
