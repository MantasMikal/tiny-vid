/**
 * Represents metadata information for a video file
 */
export type VideoMetadata = {
  duration: number;  // Duration in seconds
  width: number;     // Width in pixels
  height: number;    // Height in pixels
  size: number;      // Size in bytes
  sizeMB: number;    // Size in megabytes
};

/**
 * Extracts metadata from a video file including duration, dimensions, and file size
 * @param file - The video File object to analyze
 * @returns Promise resolving to VideoMetadata object
 * @throws Error if video loading fails
 */
export async function getVideoMetadata(file: File): Promise<VideoMetadata> {
  return new Promise((resolve, reject) => {
    const video = document.createElement("video");
    video.preload = "metadata";

    video.onloadedmetadata = () => {
      window.URL.revokeObjectURL(video.src);
      resolve({
        duration: video.duration,
        width: video.videoWidth,
        height: video.videoHeight,
        size: file.size,
        sizeMB: file.size / 1024 / 1024,
      });
    };

    video.onerror = () => {
      reject("Error loading video");
    };

    video.src = URL.createObjectURL(file);
  });
}
