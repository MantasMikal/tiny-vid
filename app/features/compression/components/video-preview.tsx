import React, { useRef, useEffect } from 'react'
import { ReactCompareSlider } from 'react-compare-slider'

interface PreviewProps {
  videoPreview: {
    original: File | Blob
    compressed: File | Blob
  }
}

function PreviewComponent(props: PreviewProps) {
  const { videoPreview } = props
  const { original, compressed } = videoPreview

  const originalVideoRef = useRef<HTMLVideoElement>(null)
  const compressedVideoRef = useRef<HTMLVideoElement>(null)

  useEffect(() => {
    const originalVideo = originalVideoRef.current
    const compressedVideo = compressedVideoRef.current
    let originalEnded = false
    let compressedEnded = false

    if (originalVideo && compressedVideo) {
      const handleReady = () => {
        if (originalVideo.readyState === 4 && compressedVideo.readyState === 4) {
          originalVideo.play()
          compressedVideo.play()
        }
      }

      originalVideo.addEventListener('loadeddata', handleReady)
      compressedVideo.addEventListener('loadeddata', handleReady)

      const tryToPlay = () => {
        if (!originalEnded || !compressedEnded) return
        originalVideo.play()
        compressedVideo.play()
        originalEnded = false
        compressedEnded = false
      }

      const handleFirstEnded = () => {
        originalEnded = true
        tryToPlay()
      }

      const handleSecondEnded = () => {
        compressedEnded = true
        tryToPlay()
      }

      originalVideo.addEventListener('ended', handleFirstEnded)
      compressedVideo.addEventListener('ended', handleSecondEnded)

      return () => {
        originalVideo.removeEventListener('loadeddata', handleReady)
        compressedVideo.removeEventListener('loadeddata', handleReady)
        originalVideo.removeEventListener('ended', handleFirstEnded)
        compressedVideo.removeEventListener('ended', handleSecondEnded)
      }
    }
  }, [])

  return (
    <ReactCompareSlider
      className="w-full h-full"
      itemOne={<Video src={compressed} ref={compressedVideoRef} />}
      itemTwo={<Video src={original} ref={originalVideoRef} />}
    />
  )
}

const Video = React.forwardRef<HTMLVideoElement, { src: File | Blob }>((props, ref) => {
  const { src } = props
  const videoSrc = React.useMemo(() => {
    const blobUrl = URL.createObjectURL(src)
    return blobUrl
  }, [src])

  return (
    <video
      ref={ref}
      muted
      className="w-full h-full object-contain"
      src={videoSrc}
      onError={(e) => {
        console.error('Video element error:', e)
        const videoElement = e.target as HTMLVideoElement
        console.error('Video element state:', {
          error: videoElement.error,
          networkState: videoElement.networkState,
          readyState: videoElement.readyState,
          src: videoElement.src,
        })
      }}
    />
  )
})

Video.displayName = 'Video'

export const VideoPreview = React.memo(PreviewComponent)
