import { execSync } from 'child_process'
import fs from 'fs/promises'

export interface PlatformInfo {
  platform: string
  arch: string
}

// Map platform names to resource names
const PLATFORM_MAP: Record<string, string> = {
  'darwin': 'macos',
  'win32': 'windows',
  'linux': 'linux'
}

// Map architecture names
const ARCH_MAP: Record<string, string> = {
  'x64': 'x64',
  'arm64': 'arm64'
}

// Common installation paths for each platform
const COMMON_PATHS = {
  darwin: [
    '/opt/homebrew/bin/ffmpeg',  // Homebrew (Apple Silicon)
    '/usr/local/bin/ffmpeg',     // Homebrew (Intel)
    '/opt/local/bin/ffmpeg',     // MacPorts
  ],
  win32: [
    'C:\\ffmpeg\\bin\\ffmpeg.exe',
    'C:\\Program Files\\ffmpeg\\bin\\ffmpeg.exe',
  ],
  linux: [
    '/usr/bin/ffmpeg',
    '/usr/local/bin/ffmpeg',
  ]
} as const

export function getTargetPlatform(): PlatformInfo {
  const platform = PLATFORM_MAP[process.platform] || process.platform
  const arch = ARCH_MAP[process.arch] || process.arch
  return { platform, arch }
}

async function findFFmpegInPath(): Promise<string | null> {
  try {
    const command = process.platform === 'win32' ? 'where ffmpeg' : 'which ffmpeg'
    const ffmpegPath = execSync(command, { 
      encoding: 'utf8',
      env: { ...process.env, ELECTRON_RUN_AS_NODE: '1' }
    }).trim().split('\n')[0]

    if (ffmpegPath) {
      try {
        await fs.access(ffmpegPath)
        return ffmpegPath
      } catch {
        return null
      }
    }
  } catch {
    return null
  }
  return null
}

export async function getFFmpegPath(): Promise<string> {
  // First try to find FFmpeg in PATH
  const pathFFmpeg = await findFFmpegInPath()
  if (pathFFmpeg) {
    return pathFFmpeg
  }

  // Check common installation locations
  const pathsToCheck = COMMON_PATHS[process.platform as keyof typeof COMMON_PATHS] || []

  for (const ffmpegPath of pathsToCheck) {
    try {
      await fs.access(ffmpegPath)
      return ffmpegPath
    } catch {
      continue
    }
  }

  throw new Error(
    'FFmpeg not found. Please install FFmpeg on your system:\n' +
    '  - macOS: brew install ffmpeg\n' +
    '  - Linux: sudo apt install ffmpeg\n' +
    '  - Windows: Download from https://ffmpeg.org/download.html'
  )
}
