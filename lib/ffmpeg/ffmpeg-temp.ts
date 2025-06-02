import { writeFile, unlink } from 'fs/promises'
import { join } from 'path'
import { tmpdir } from 'os'

export class TempFileManager {
  private files: string[] = []

  async create(suffix: string, content?: Buffer): Promise<string> {
    const path = join(tmpdir(), `ffmpeg-${Date.now()}-${Math.random().toString(36).substr(2, 9)}-${suffix}`)
    this.files.push(path)
    if (content) {
      await writeFile(path, content)
    }
    return path
  }

  async cleanup(): Promise<void> {
    await Promise.allSettled(this.files.map((f) => unlink(f).catch(() => {})))
    this.files = []
  }
} 