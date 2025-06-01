'use client'

import React, { useState } from 'react'
import { useDropzone } from 'react-dropzone'
import { type DropzoneProps as _DropzoneProps, type DropzoneState as _DropzoneState } from 'react-dropzone'
import { cn } from '@/lib/utils'

export type DropzoneState = _DropzoneState

export interface DropzoneProps extends Omit<_DropzoneProps, 'children'> {
  containerClassName?: string
  dropZoneClassName?: string
  filesUploaded: File[]
  setFilesUploaded: (files: File[]) => void
  loading?: boolean
  instructions?: string
}

const Upload = ({ className }: { className?: string }) => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    width="24"
    height="24"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
    className={cn('lucide lucide-upload', className)}
  >
    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
    <polyline points="17 8 12 3 7 8" />
    <line x1="12" x2="12" y1="3" y2="15" />
  </svg>
)

const Dropzone = ({
  containerClassName,
  dropZoneClassName,
  filesUploaded,
  setFilesUploaded,
  loading,
  disabled,
  maxFiles,
  instructions,
  ...props
}: DropzoneProps) => {
  const dropzone = useDropzone({
    ...props,
    ...(maxFiles && { maxFiles }),
    onDrop(acceptedFiles, fileRejections) {
      // Log the files to see what we get from Electron
      setFilesUploaded([...filesUploaded, ...acceptedFiles])

      if (fileRejections.length > 0) {
        let _errorMessage = `Could not upload ${fileRejections[0].file.name}`
        if (fileRejections.length > 1) _errorMessage = _errorMessage + `, and ${fileRejections.length - 1} other files.`
        setErrorMessage(_errorMessage)
      } else {
        setErrorMessage('')
      }
    },
  })

  const [errorMessage, setErrorMessage] = useState<string>()

  return (
    <div className={cn('relative flex flex-col gap-2', containerClassName)}>
      {!loading && (
        <div
          {...dropzone.getRootProps()}
          className={cn(
            'relative flex justify-center items-center w-full h-32 border-dashed border-2 border-gray-200 rounded-lg transition-all select-none',
            disabled && 'opacity-50 cursor-not-allowed',
            !disabled && 'hover:bg-accent hover:text-accent-foreground cursor-pointer',
            dropZoneClassName
          )}
        >
          <input {...dropzone.getInputProps()} disabled={disabled} />
          <div className="flex items-center flex-col gap-1.5 p-4">
            <div className="flex items-center flex-row gap-0.5 text-sm font-medium">
              <Upload className="mr-2 h-4 w-4" /> Drop your file here!
            </div>
            {instructions && <div className="text-center text-xs text-gray-400 font-medium">{instructions}</div>}
          </div>
        </div>
      )}
      {errorMessage && <span className="text-xs text-red-600 mt-3">{errorMessage}</span>}
    </div>
  )
}

export default Dropzone
