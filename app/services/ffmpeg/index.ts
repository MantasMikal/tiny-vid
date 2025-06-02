import { TranscodeOptions, TranscodeOutput, PreviewOutput, DEFAULTS } from './types';

export class FFmpegService {
  async transcode(file: File, options: TranscodeOptions): Promise<TranscodeOutput> {
    const buffer = await file.arrayBuffer();
    const result = await window.ffmpeg.transcode({
      file: buffer,
      name: file.name,
      options,
    });
    
    if (!result) {
      const abortError = new Error('Process was terminated');
      abortError.name = 'AbortError';
      throw abortError;
    }
    return {
      file: new Blob([result.file], { type: 'video/mp4' }),
      name: result.name,
    };
  }

  async generatePreview(file: File, options: TranscodeOptions): Promise<PreviewOutput> {
    const buffer = await file.arrayBuffer();
    const result = await window.ffmpeg.generatePreview({
      file: buffer,
      name: file.name,
      options: { ...options, previewDuration: options.previewDuration ?? DEFAULTS.PREVIEW_DURATION },
    });
    
    if (!result) {
      const abortError = new Error('Process was terminated');
      abortError.name = 'AbortError';
      throw abortError;
    }
    return {
      original: new Blob([result.original], { type: 'video/mp4' }),
      compressed: new Blob([result.compressed], { type: 'video/mp4' }),
      estimatedSize: result.estimatedSize,
    };
  }

  terminate(): void {
    window.ffmpeg.terminate();
  }
}

export * from './types'; 