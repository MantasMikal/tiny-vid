'use client'

import { useCallback, useReducer, useRef, useEffect } from 'react'
import { FFmpegService } from '@/app/services/ffmpeg'
import { TranscodeOptions, PreviewOutput, TranscodeOutput } from '@/app/services/ffmpeg/types'

export type FFmpegState = {
  isTranscoding: boolean
  isGeneratingPreview: boolean
  progress: number
  error: { type: string; message: string } | null
}

export type FFmpegAction =
  | { type: 'PREVIEW_START' }
  | { type: 'PREVIEW_SUCCESS' }
  | { type: 'PREVIEW_FAILURE'; error: string }
  | { type: 'TRANSCODE_START' }
  | { type: 'TRANSCODE_PROGRESS'; progress: number }
  | { type: 'TRANSCODE_SUCCESS' }
  | { type: 'TRANSCODE_FAILURE'; error: string }
  | { type: 'TERMINATE' }

/**
 * Reducer function for managing FFmpeg state transitions
 * @param state - Current FFmpeg state
 * @param action - Action to perform on the state
 * @returns Updated FFmpeg state
 */
function ffmpegReducer(state: FFmpegState, action: FFmpegAction): FFmpegState {
  switch (action.type) {
    case 'PREVIEW_START':
      return { ...state, isGeneratingPreview: true, error: null }
    case 'PREVIEW_SUCCESS':
      return { ...state, isGeneratingPreview: false }
    case 'PREVIEW_FAILURE':
      return { ...state, isGeneratingPreview: false, error: { type: 'Preview Error', message: action.error } }
    case 'TRANSCODE_START':
      return { ...state, isTranscoding: true, progress: 0, error: null }
    case 'TRANSCODE_PROGRESS':
      return { ...state, progress: action.progress }
    case 'TRANSCODE_SUCCESS':
      return { ...state, isTranscoding: false, progress: 1 }
    case 'TRANSCODE_FAILURE':
      return { ...state, isTranscoding: false, error: { type: 'Transcode Error', message: action.error } }
    case 'TERMINATE':
      return { ...state, isTranscoding: false, isGeneratingPreview: false, progress: 0 }
    default:
      return state
  }
}

const initialState: FFmpegState = {
  isTranscoding: false,
  isGeneratingPreview: false,
  progress: 0,
  error: null,
}

/**
 * Hook for managing FFmpeg video processing operations
 * @returns Object containing FFmpeg state and methods for video processing
 * - state: Current processing state and progress
 * - abort: Function to cancel current operation
 * - transcode: Function to convert video to different format/quality
 * - generateVideoPreview: Function to create preview version of video
 */
export const useFfmpeg = () => {
  const [state, dispatch] = useReducer(ffmpegReducer, initialState)
  const ffmpegServiceRef = useRef<FFmpegService | null>(null)

  // Initialize FFmpeg service
  useEffect(() => {
    ffmpegServiceRef.current = new FFmpegService()
    return () => {
      ffmpegServiceRef.current?.terminate()
    }
  }, [])

  // Set up event listeners
  useEffect(() => {
    if (!ffmpegServiceRef.current) return

    const cleanupProgress = window.ffmpeg.onProgress((progress) => {
      dispatch({ type: 'TRANSCODE_PROGRESS', progress })
    })

    const cleanupError = window.ffmpeg.onError((error) => {
      if (state.isTranscoding) {
        dispatch({ type: 'TRANSCODE_FAILURE', error })
      } else if (state.isGeneratingPreview) {
        dispatch({ type: 'PREVIEW_FAILURE', error })
      }
    })

    const cleanupComplete = window.ffmpeg.onComplete(() => {
      if (state.isTranscoding) {
        dispatch({ type: 'TRANSCODE_SUCCESS' })
      } else if (state.isGeneratingPreview) {
        dispatch({ type: 'PREVIEW_SUCCESS' })
      }
    })

    return () => {
      cleanupProgress()
      cleanupError()
      cleanupComplete()
    }
  }, [state.isTranscoding, state.isGeneratingPreview])

  /**
   * Terminates the FFmpeg service and resets the state
   */
  const terminate = useCallback(() => {
    if (!ffmpegServiceRef.current) return
    ffmpegServiceRef.current.terminate()
    dispatch({ type: 'TERMINATE' })
  }, [])

  /**
   * Transcodes a video file according to specified options
   * @param file - Video file to transcode
   * @param options - Transcoding configuration options
   * @returns Promise resolving to transcode output or null if operation fails/aborts
   */
  const transcode = useCallback(
    async (file: File, options: TranscodeOptions): Promise<TranscodeOutput | null> => {
      if (!ffmpegServiceRef.current) return null
      dispatch({ type: 'TRANSCODE_START' })
      try {
        return await ffmpegServiceRef.current.transcode(file, options)
      } catch (error) {
        if (error instanceof Error && error.name === 'AbortError') {
          return null
        }
        dispatch({ type: 'TRANSCODE_FAILURE', error: (error as Error).message })
        terminate()
        return null
      }
    },
    [terminate]
  )

  /**
   * Generates a preview version of a video file
   * @param file - Video file to generate preview from
   * @param options - Preview generation configuration options
   * @returns Promise resolving to preview output or null if operation fails/aborts
   */
  const generateVideoPreview = useCallback(
    async (file: File, options: TranscodeOptions): Promise<PreviewOutput | null> => {
      if (!ffmpegServiceRef.current) return null
      dispatch({ type: 'PREVIEW_START' })
      try {
        return await ffmpegServiceRef.current.generatePreview(file, options)
      } catch (error) {
        if (error instanceof Error && error.name === 'AbortError') {
          return null
        }
        dispatch({ type: 'PREVIEW_FAILURE', error: (error as Error).message })
        terminate()
        return null
      }
    },
    [terminate]
  )

  return {
    state,
    terminate,
    transcode,
    generateVideoPreview,
  }
}
