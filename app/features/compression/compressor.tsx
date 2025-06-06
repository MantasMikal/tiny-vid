'use client'

import { useRef, useState } from 'react'
import { motion, AnimatePresence } from 'framer-motion'

import { Button } from '@/app/components/ui/button'
import Dropzone from '@/app/components/ui/dropzone'
import { ReloadIcon, TrashIcon, ExclamationTriangleIcon, StopIcon } from '@radix-ui/react-icons'
import { Progress } from '@/app/components/ui/progress'
import { Spinner } from '@/app/components/ui/spinner'
import { Alert, AlertDescription, AlertTitle } from '@/app/components/ui/alert'
import { getVideoMetadata, VideoMetadata } from '@/app/features/compression/lib/get-video-metadata'
import { Separator } from '@/app/components/ui/separator'
import { CompressionOptions, VideoSettings } from '@/app/features/compression/components/video-settings'
import { useFfmpeg } from '@/app/features/compression/hooks/use-ffmpeg'
import { VideoMetadataDisplay } from './components/video-metadata-display'
import { VideoPreview } from './components/video-preview'
import { ScrollArea } from '@/app/components/ui/scroll-area'
import { downloadFile } from './lib/download-file'

export default function Compressor() {
  const [files, setFiles] = useState<File[]>([])
  const [videoPreview, setVideoPreview] = useState<{
    original: Blob
    compressed: Blob
  } | null>(null)
  const [videoUploading, setVideoUploading] = useState(false)
  const [estimatedSize, setEstimatedSize] = useState<number | null>(null)
  const [videoMetadata, setVideoMetadata] = useState<VideoMetadata | null>(null)
  const debouncePreviewTimerRef = useRef<NodeJS.Timeout | null>(null)
  const [cOptions, setCOptions] = useState<CompressionOptions>({
    quality: 75,
    preset: 'fast',
    fps: 30,
    scale: 1,
    removeAudio: false,
    codec: 'libx264',
    generatePreview: true,
    previewDuration: 3,
    tune: undefined,
  })

  const {
    state: { error, isTranscoding, isGeneratingPreview, progress },
    generateVideoPreview,
    transcode,
    terminate,
  } = useFfmpeg()

  const handleTranscode = async () => {
    const file = files[0]
    if (!file) {
      console.error('No file to transcode')
      return
    }

    const result = await transcode(file, cOptions)
    if (!result) return
    const { file: output, name } = result
    downloadFile(output, name)
  }

  const handleFileAccepted = async (acceptedFiles: File[]) => {
    if (acceptedFiles.length === 0) {
      return
    }

    setFiles(acceptedFiles)
    const file = acceptedFiles[0]

    if (file) {
      setVideoUploading(true)
      await Promise.all([
        getVideoMetadata(file).then((metadata) => setVideoMetadata(metadata)),
        handleGeneratePreview(file, cOptions),
      ])

      setVideoUploading(false)
    }
  }

  const handleGeneratePreview = async (file: File, options: CompressionOptions) => {
    try {
      const result = await generateVideoPreview(file, options)
      if (!result) return // Aborted - preserve existing preview
      const { original, compressed, estimatedSize: size } = result
      setEstimatedSize(size)
      setVideoPreview({ original, compressed })
    } catch (error) {
      if (error instanceof Error && error.name === 'AbortError') {
        // Process was aborted - preserve existing preview, don't log as error
        return
      }
      console.error('Error estimating output size:', error)
    }
  }

  const debouncedGeneratePreview = (options: CompressionOptions) => {
    if (files.length > 0) {
      if (debouncePreviewTimerRef.current) {
        clearTimeout(debouncePreviewTimerRef.current)
      }

      debouncePreviewTimerRef.current = setTimeout(async () => {
        await handleGeneratePreview(files[0], options)
      }, 300)
    }
  }

  const handleOptionsChange = (options: CompressionOptions) => {
    setCOptions(options)
    if (!options.generatePreview) return
    debouncedGeneratePreview(options)
  }

  const isDisabled = isTranscoding || isGeneratingPreview
  const isWorking = isTranscoding || isGeneratingPreview

  return (
    <div className="grow flex flex-col gap-4 h-full w-full overflow-y-auto md:overflow-hidden">
      <div className="grow grid items-start md:grid-cols-3 gap-4 w-full h-full mx-auto md:overflow-hidden">
        <div className="relative flex flex-col gap-2 md:col-span-2 border p-2 rounded-md bg-card h-full min-h-[300px]">
          <div className="relative flex items-center justify-center h-full">
            {files.length === 0 && (
              <Dropzone
                containerClassName="w-full h-full"
                dropZoneClassName="w-full h-full"
                filesUploaded={files}
                setFilesUploaded={handleFileAccepted}
                disabled={isDisabled}
                maxFiles={1}
                accept={{
                  'video/mp4': ['.mp4'],
                  'video/mpeg': ['.mpeg'],
                  'video/webm': ['.webm'],
                  'video/quicktime': ['.mov'],
                  'video/3gpp': ['.3gp'],
                  'video/x-msvideo': ['.avi'],
                  'video/x-flv': ['.flv'],
                  'video/x-matroska': ['.mkv'],
                  'video/ogg': ['.ogg'],
                  'video/avi': ['.avi'],
                }}
              />
            )}
            {videoUploading && <Spinner className="absolute inset-0 m-auto w-12 h-12" />}
            {files.length > 0 && videoPreview && !videoUploading && (
              <div className="relative w-full h-full flex bg-black rounded-md md:overflow-hidden">
                <VideoPreview videoPreview={videoPreview} />
                <Button size="icon" onClick={() => setFiles([])} className="absolute top-4 right-4">
                  <TrashIcon className="h-5 w-5" />
                </Button>
                {!videoPreview && isGeneratingPreview && (
                  <div className="absolute inset-0 bg-black bg-opacity-50 flex items-center justify-center">
                    <Spinner className="border-white" />
                  </div>
                )}
                {videoPreview && isGeneratingPreview && (
                  <AnimatePresence>
                    <motion.div
                      className="absolute bottom-1 left-1 backdrop-blur p-1 px-2 rounded-md bg-black bg-opacity-50 flex items-center gap-2"
                      initial={{ opacity: 0, transform: 'translateY(10%)' }}
                      animate={{ opacity: 1, transform: 'translateY(0%)' }}
                      exit={{ opacity: 0, transform: 'translateY(10%)' }}
                    >
                      <Spinner className="border-white w-4 h-4 border-2" />
                      <span className="text-sm text-white">Generating preview</span>
                    </motion.div>
                  </AnimatePresence>
                )}
                {isTranscoding && <Progress className="absolute w-full bottom-0" value={progress * 100} />}
              </div>
            )}
          </div>
          <AnimatePresence>
            {error && (
              <motion.div
                className="absolute bottom-0 left-0 p-3 w-full z-10"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                transition={{ type: 'spring', stiffness: 100 }}
              >
                <Alert className="bg-black" variant="destructive">
                  <ExclamationTriangleIcon className="h-5 w-5" />
                  <AlertTitle>{error.type || 'Error'}</AlertTitle>
                  <AlertDescription>{error.message || 'An unexpected error occurred'}</AlertDescription>
                </Alert>
              </motion.div>
            )}
          </AnimatePresence>
        </div>
        <aside className="flex flex-col col-span-1 gap-4 h-full md:overflow-hidden">
          <div className="flex flex-col border p-1 bg-card rounded-md md:overflow-hidden">
            <ScrollArea className="p-2 h-full">
              <div className="flex p-1 flex-col gap-2 grow">
                <h2 className="text-xl font-semibold">Settings</h2>
                <VideoSettings isDisabled={isDisabled} cOptions={cOptions} onOptionsChange={handleOptionsChange} />
              </div>
            </ScrollArea>
          </div>
          <AnimatePresence>
            {files.length > 0 && (
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="flex flex-col gap-2 border bg-card p-4 rounded-md"
              >
                <h2 className="text-xl font-semibold">Details</h2>
                {videoMetadata && (
                  <VideoMetadataDisplay
                    videoMetadata={videoMetadata}
                    cOptions={cOptions}
                    estimatedSize={estimatedSize}
                  />
                )}
                <Separator />
                <div className="flex w-full justify-evenly flex-wrap gap-2">
                  <Button
                    className="flex-1"
                    onClick={isWorking ? terminate : handleTranscode}
                    disabled={files.length === 0}
                  >
                    {isWorking && <StopIcon className="mr-2 h-4 w-4" />}
                    {isWorking ? 'Stop' : 'Compress'}
                  </Button>
                  {!cOptions.generatePreview && (
                    <Button
                      className="flex-1"
                      variant="secondary"
                      onClick={() => handleGeneratePreview(files[0], cOptions)}
                      disabled={isDisabled || isGeneratingPreview}
                    >
                      {isGeneratingPreview && <ReloadIcon className="mr-2 h-4 w-4 animate-spin" />}
                      {isGeneratingPreview ? 'Processing' : 'Generate Preview'}
                    </Button>
                  )}
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </aside>
      </div>
    </div>
  )
}
