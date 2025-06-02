export class FFmpegCommandBuilder {
  private args: string[] = ['-threads', '0', '-progress', 'pipe:1']
  private codec: string

  constructor(codec: string) {
    this.codec = codec
  }

  private addCodecArgs(): void {
    // Add codec and codec-specific optimizations
    this.args.push('-c:v', this.codec)

    // Add codec-specific optimizations
    if (this.codec === 'libaom-av1') {
      this.args.push('-row-mt', '1', '-tile-rows', '1', '-tile-columns', '3', '-cpu-used', '3', '-aq-mode', '1')
    }
  }

  input(path: string): this {
    this.args.push('-i', path)
    this.addCodecArgs()
    return this
  }

  output(path: string): this {
    this.args.push(path)
    return this
  }

  crf(crf: number): this {
    this.args.push('-crf', crf.toString())
    return this
  }

  constrainedCrf(crf: number, maxBitrate: number): this {
    this.args.push('-crf', crf.toString(), '-maxrate', `${maxBitrate}k`, '-bufsize', `${maxBitrate * 2}k`)
    return this
  }

  audio(enabled: boolean): this {
    if (enabled) {
      this.args.push('-c:a', 'aac', '-b:a', '128k')
    } else {
      this.args.push('-an')
    }
    return this
  }

  scale(factor: number): this {
    if (factor < 1) {
      this.args.push('-vf', `scale=round(iw*${factor}/2)*2:-2`)
    }
    return this
  }

  preset(preset: string): this {
    this.args.push('-preset', preset)
    return this
  }

  fps(fps: number): this {
    this.args.push('-r', fps.toString())
    return this
  }

  fastStart(): this {
    this.args.push('-movflags', '+faststart')
    return this
  }

  tune(tuneValue: string | undefined): this {
    // Only apply tune parameters to codecs that support them (x264 and x265, but not AV1)
    if (this.codec === 'libaom-av1') {
      return this
    }

    if (tuneValue && tuneValue !== 'none') {
      this.args.push('-tune', tuneValue)
    }

    return this
  }

  build(): string[] {
    return this.args
  }
} 