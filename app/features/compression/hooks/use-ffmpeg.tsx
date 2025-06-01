'use client'

import { useCallback, useReducer, useMemo, useRef, useEffect } from 'react'
import { FFmpegService } from '@/app/services/ffmpeg'
import { TranscodeOptions, PreviewOutput, TranscodeOutput } from '@/app/services/ffmpeg/types'

export type FFmpegState = {
  isLoaded: boolean
  isLoading: boolean
  isTranscoding: boolean
  isGeneratingPreview: boolean
  progress: number
  error: { type: string; message: string } | null
}

export type FFmpegAction =
  | { type: 'LOAD_START' }
  | { type: 'LOAD_SUCCESS' }
  | { type: 'LOAD_FAILURE'; error: string }
  | { type: 'PREVIEW_START' }
  | { type: 'PREVIEW_SUCCESS' }
  | { type: 'PREVIEW_FAILURE'; error: string }
  | { type: 'TRANSCODE_START' }
  | { type: 'TRANSCODE_PROGRESS'; progress: number }
  | { type: 'TRANSCODE_SUCCESS' }
  | { type: 'TRANSCODE_FAILURE'; error: string }
  | { type: 'ABORT' }
  | { type: 'TERMINATE' }

/**
 * Reducer function for managing FFmpeg state transitions
 * @param state - Current FFmpeg state
 * @param action - Action to perform on the state
 * @returns Updated FFmpeg state
 */
function ffmpegReducer(state: FFmpegState, action: FFmpegAction): FFmpegState {
  switch (action.type) {
    case 'LOAD_START':
      return { ...state, isLoading: true }
    case 'LOAD_SUCCESS':
      return { ...state, isLoaded: true, isLoading: false }
    case 'LOAD_FAILURE':
      return { ...state, isLoading: false, error: { type: 'Load Error', message: action.error } }
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
    case 'ABORT':
      return { ...state, isTranscoding: false, isGeneratingPreview: false, progress: 0 }
    case 'TERMINATE':
      return { ...state, isLoaded: false, isLoading: false, isTranscoding: false, isGeneratingPreview: false }
    default:
      return state
  }
}

const initialState: FFmpegState = {
  isLoaded: false,
  isLoading: false,
  isTranscoding: false,
  isGeneratingPreview: false,
  progress: 0,
  error: null,
}

/**
 * Hook for managing FFmpeg video processing operations
 * @returns Object containing FFmpeg state and methods for video processing
 * - state: Current processing state and progress
 * - load: Function to initialize FFmpeg
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
   * Initializes FFmpeg service
   */
  const load = useCallback(async () => {
    if (!ffmpegServiceRef.current) return
    dispatch({ type: 'LOAD_START' })
    try {
      await ffmpegServiceRef.current.load()
      dispatch({ type: 'LOAD_SUCCESS' })
    } catch (error) {
      dispatch({ type: 'LOAD_FAILURE', error: (error as Error).message })
    }
  }, [])

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
          dispatch({ type: 'ABORT' })
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
          dispatch({ type: 'ABORT' })
          return null
        }
        dispatch({ type: 'PREVIEW_FAILURE', error: (error as Error).message })
        terminate()
        return null
      }
    },
    [terminate]
  )

  return useMemo(
    () => ({
      ...state,
      load,
      transcode,
      generateVideoPreview,
      terminate,
    }),
    [state, load, transcode, generateVideoPreview, terminate]
  )
}
