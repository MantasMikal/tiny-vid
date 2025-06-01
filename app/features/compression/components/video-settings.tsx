'use client'

import { motion, AnimatePresence } from 'framer-motion'
import { Label } from '@/app/components/ui/label'
import { Slider } from '@/app/components/ui/slider'
import { Input } from '@/app/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/app/components/ui/select'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/app/components/ui/tabs'
import { ToggleGroup, ToggleGroupItem } from '@/app/components/ui/toggle-group'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/app/components/ui/tooltip'
import { BikeIcon, CarFrontIcon, CookingPotIcon, InfoIcon, LucideIcon, RocketIcon } from 'lucide-react'
import { useState } from 'react'
import { Checkbox } from '@/app/components/ui/checkbox'

export type CompressionOptions = {
  quality: number
  maxBitrate?: number
  preset: (typeof presets)[number]['value']
  fps: number
  scale: number
  removeAudio: boolean
  codec: string
  generatePreview?: boolean
  previewDuration?: number
  tune?: string
}

type BasicPresets = 'basic' | 'super' | 'ultra' | 'cooked'
type TabOptions = 'basic' | 'advanced'

type ConfigOption = {
  value: string
  icon: LucideIcon
  title: string
  description: string
  options: CompressionOptions
}

const toggleConfig: ConfigOption[] = [
  {
    value: 'basic',
    icon: BikeIcon,
    title: 'Basic',
    description: 'Basic compression with minimal loss in quality',
    options: {
      quality: 90,
      preset: 'fast',
      fps: 30,
      scale: 1,
      removeAudio: false,
      codec: 'libx264',
      generatePreview: true,
      tune: undefined,
    },
  },
  {
    value: 'super',
    icon: CarFrontIcon,
    title: 'Medium',
    description: 'Medium compression with some loss in quality',
    options: {
      quality: 75,
      preset: 'fast',
      fps: 30,
      scale: 1,
      removeAudio: false,
      codec: 'libx264',
      generatePreview: true,
      tune: undefined,
    },
  },
  {
    value: 'ultra',
    icon: RocketIcon,
    title: 'Strong',
    description: 'Strong compression with loss in quality',
    options: {
      quality: 60,
      preset: 'fast',
      fps: 30,
      scale: 1,
      removeAudio: false,
      codec: 'libx264',
      generatePreview: true,
      tune: undefined,
    },
  },
  {
    value: 'cooked',
    icon: CookingPotIcon,
    title: 'Cooked',
    description: 'Deep fried with extra crunch',
    options: {
      quality: 40,
      preset: 'fast',
      fps: 30,
      scale: 1,
      removeAudio: false,
      codec: 'libx264',
      generatePreview: true,
      tune: undefined,
    },
  },
] as const

export const codecs = [
  {
    name: 'H.264 (Best compatibility)',
    value: 'libx264',
  },
  {
    name: 'H.265 (Better compression)',
    value: 'libx265',
  },
  {
    name: 'AV1 (Best, very slow)',
    value: 'libaom-av1',
  },
] as const

export const tuneOptions = [
  { name: 'None (Default)', value: 'none' },
  { name: 'Film - High quality movie content', value: 'film' },
  { name: 'Animation - Animated content', value: 'animation' },
  { name: 'Grain - Preserve film grain', value: 'grain' },
  { name: 'Still Image - Optimize for still images', value: 'stillimage' },
  { name: 'Fast Decode - Optimize for fast decoding', value: 'fastdecode' },
  { name: 'Zero Latency - Streaming/low latency', value: 'zerolatency' },
  { name: 'PSNR - Optimize for PSNR metric', value: 'psnr' },
  { name: 'SSIM - Optimize for SSIM metric', value: 'ssim' },
] as const

export const maxBitratePresets = [
  { name: 'No limit', value: 'none' },
  { name: 'Low (500 kbps)', value: 500 },
  { name: 'Medium (1000 kbps)', value: 1000 },
  { name: 'High (2000 kbps)', value: 2000 },
  { name: 'Very High (4000 kbps)', value: 4000 },
  { name: 'Ultra (8000 kbps)', value: 8000 },
  { name: 'Custom', value: 'custom' },
] as const

export const presets = [
  {
    name: 'Ultra Fast',
    value: 'ultrafast',
  },
  {
    name: 'Super Fast',
    value: 'superfast',
  },
  {
    name: 'Very Fast',
    value: 'veryfast',
  },
  {
    name: 'Faster',
    value: 'faster',
  },
  {
    name: 'Fast',
    value: 'fast',
  },
  {
    name: 'Medium',
    value: 'medium',
  },
  {
    name: 'Slow',
    value: 'slow',
  },
] as const

const MotionTabsContent = motion.create(TabsContent)

interface VideoSettingsProps {
  isDisabled: boolean
  cOptions: CompressionOptions
  onOptionsChange: (options: CompressionOptions) => void
}

export function VideoSettings({ isDisabled, cOptions, onOptionsChange }: VideoSettingsProps) {
  const [activeTab, setActiveTab] = useState<TabOptions>('basic')
  const [basicPreset, setBasicPreset] = useState<BasicPresets>('super')
  const [showCustomMaxBitrate, setShowCustomMaxBitrate] = useState(false)

  const handleQualityChange = (value: number) => {
    onOptionsChange({
      ...cOptions,
      quality: value,
    })
  }

  const handleMaxBitrateChange = (value: number | undefined) => {
    onOptionsChange({
      ...cOptions,
      maxBitrate: value,
    })
  }

  const handleMaxBitratePresetChange = (value: string | number) => {
    if (value === 'custom') {
      setShowCustomMaxBitrate(true)
    } else if (value === 'none') {
      handleMaxBitrateChange(undefined)
      setShowCustomMaxBitrate(false)
    } else {
      const numericValue = typeof value === 'number' ? value : parseInt(value.toString())
      handleMaxBitrateChange(numericValue)
      setShowCustomMaxBitrate(false)
    }
  }

  const handleScaleChange = (value: number) => {
    onOptionsChange({
      ...cOptions,
      scale: value,
    })
  }

  const handlePresetChange = (value: string) => {
    onOptionsChange({
      ...cOptions,
      preset: value as CompressionOptions['preset'],
    })
  }

  const handleFpsChange = (value: number | string) => {
    if (typeof value === 'number') {
      onOptionsChange({
        ...cOptions,
        fps: value,
      })
    }
  }

  const handleAudioChange = (value: boolean) => {
    onOptionsChange({
      ...cOptions,
      removeAudio: value,
    })
  }

  const handlePreviewDurationChange = (value: number) => {
    onOptionsChange({
      ...cOptions,
      previewDuration: value,
    })
  }

  const handlePreviewEnabledChange = (value: boolean) => {
    onOptionsChange({
      ...cOptions,
      generatePreview: value,
    })
  }

  const handleCodecChange = (value: string) => {
    onOptionsChange({
      ...cOptions,
      codec: value,
    })
  }

  const handleTuneChange = (value: string) => {
    onOptionsChange({
      ...cOptions,
      tune: value === 'none' ? undefined : value,
    })
  }

  const handleBasicPresetChange = (value: BasicPresets) => {
    if (!value) return
    const preset = toggleConfig.find((config) => config.value === value)
    setBasicPreset(value)
    if (preset) {
      onOptionsChange({
        ...cOptions,
        ...preset.options,
      })
    }
  }

  return (
    <TooltipProvider>
      <Tabs value={activeTab} className="w-full" onValueChange={(value) => setActiveTab(value as TabOptions)}>
        <TabsList className="grid w-full mb-4 grid-cols-2">
          <TabsTrigger value="basic">Basic</TabsTrigger>
          <TabsTrigger value="advanced">Advanced</TabsTrigger>
        </TabsList>
        <AnimatePresence initial={false}>
          {activeTab === 'basic' && (
            <MotionTabsContent
              key="basic"
              className="flex flex-col gap-4"
              value="basic"
              initial={{
                opacity: 0,
                translateX: 100,
              }}
              animate={{ opacity: 1, translateX: 0 }}
              exit={{
                opacity: 0,
                translateX: -100,
              }}
            >
              <div className="flex flex-col gap-2">
                <h3 className="text-base font-bold">Preset</h3>
                <ToggleGroup
                  value={basicPreset}
                  onValueChange={handleBasicPresetChange}
                  disabled={isDisabled}
                  className="w-full flex-col items-start gap-2"
                  type="single"
                  size="lg"
                >
                  {toggleConfig.map((config) => (
                    <ToggleItem key={config.value} {...config} />
                  ))}
                </ToggleGroup>
              </div>
              <div className="flex flex-col gap-2">
                <h3 className="text-base font-bold">Audio</h3>
                <div className="flex items-center space-x-2">
                  <Checkbox
                    id="removeAudio"
                    disabled={isDisabled}
                    checked={cOptions.removeAudio}
                    onCheckedChange={(checked) => handleAudioChange(!!checked)}
                  />
                  <label
                    htmlFor="removeAudio"
                    className="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70"
                  >
                    Remove soundtrack
                  </label>
                </div>
              </div>
            </MotionTabsContent>
          )}
          {activeTab === 'advanced' && (
            <MotionTabsContent
              key="advanced"
              className="flex flex-col gap-4"
              value="advanced"
              initial={{
                opacity: 0,
                translateX: -100,
              }}
              animate={{ opacity: 1, translateX: 0 }}
              exit={{
                opacity: 0,
                translateX: 100,
              }}
            >
              <div className="flex flex-col gap-2">
                <TooltipLabel
                  className="text-base font-bold"
                  htmlFor="codec"
                  tooltip="Choose the video compression codec. H.264 has best compatibility, H.265 provides better compression, AV1 is most efficient but slowest."
                >
                  Codec
                </TooltipLabel>
                <Select
                  value={cOptions.codec}
                  disabled={isDisabled}
                  onValueChange={(value) => handleCodecChange(value)}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {codecs.map((codec) => (
                      <SelectItem key={codec.value} value={codec.value}>
                        {codec.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="flex flex-col gap-2">
                <TooltipLabel
                  className="text-base font-bold"
                  htmlFor="quality"
                  tooltip="Quality level using CRF (Constant Rate Factor). Lower values = better quality but larger files."
                >
                  Quality (CRF)
                </TooltipLabel>
                <Slider
                  disabled={isDisabled}
                  name="quality"
                  id="quality"
                  min={1}
                  max={100}
                  step={1}
                  defaultValue={[cOptions.quality]}
                  value={[cOptions.quality]}
                  onValueChange={(value) => {
                    handleQualityChange(value[0])
                  }}
                />
              </div>
              <div className="flex flex-col gap-2">
                <TooltipLabel
                  className="text-base font-bold"
                  htmlFor="maxBitrate"
                  tooltip="Optional maximum bitrate constraint for CRF. Set to prevent bitrate spikes. Leave unset for pure CRF encoding."
                >
                  Max Bitrate Constraint
                </TooltipLabel>
                <Select
                  value={
                    showCustomMaxBitrate
                      ? 'custom'
                      : cOptions.maxBitrate === undefined
                        ? 'none'
                        : maxBitratePresets.find((p) => p.value === cOptions.maxBitrate)?.value?.toString() || 'custom'
                  }
                  disabled={isDisabled}
                  onValueChange={handleMaxBitratePresetChange}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {maxBitratePresets.map((preset) => (
                      <SelectItem key={preset.value} value={preset.value.toString()}>
                        {preset.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                {showCustomMaxBitrate && (
                  <Input
                    disabled={isDisabled}
                    onChange={(e) => {
                      const value = parseInt(e.target.value)
                      handleMaxBitrateChange(isNaN(value) || value <= 0 ? undefined : value)
                    }}
                    value={cOptions.maxBitrate || ''}
                    type="number"
                    min={100}
                    max={50000}
                    placeholder="e.g. 2000"
                  />
                )}
              </div>
              <div className="flex flex-col gap-2">
                <TooltipLabel
                  className="text-base font-bold"
                  htmlFor="preset"
                  tooltip="Compression speed. Slower presets provide better quality but take longer to process."
                >
                  Encoding Preset
                </TooltipLabel>
                <Select
                  value={cOptions.preset}
                  disabled={isDisabled}
                  onValueChange={(value) => handlePresetChange(value)}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {presets.map((preset) => (
                      <SelectItem key={preset.value} value={preset.value}>
                        {preset.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="flex flex-col gap-2">
                <TooltipLabel
                  className="text-base font-bold"
                  htmlFor="tune"
                  tooltip="Tune options optimize the encoder for specific content types (screen capture, film, animation, etc.)."
                >
                  Tune
                </TooltipLabel>
                <Select
                  value={cOptions.tune ?? 'none'}
                  disabled={isDisabled}
                  onValueChange={(value) => handleTuneChange(value)}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {tuneOptions.map((tune) => (
                      <SelectItem key={tune.value} value={tune.value}>
                        {tune.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="flex flex-col gap-2">
                <TooltipLabel
                  className="text-base font-bold"
                  htmlFor="scale"
                  tooltip="Scale video resolution. 1.0 = original size, 0.5 = half size. Greatly affects file size."
                >
                  Resolution Scale
                </TooltipLabel>
                <Slider
                  disabled={isDisabled}
                  name="scale"
                  id="scale"
                  min={0.25}
                  max={1}
                  step={0.05}
                  defaultValue={[cOptions.scale]}
                  value={[cOptions.scale]}
                  onValueChange={(value) => handleScaleChange(value[0])}
                />
              </div>
              <div className="flex flex-col gap-2">
                <TooltipLabel
                  className="text-base font-bold"
                  htmlFor="fps"
                  tooltip="Frames per second. Lower FPS reduces file size."
                >
                  Frame Rate (FPS)
                </TooltipLabel>
                <Input
                  disabled={isDisabled}
                  onChange={(e) => handleFpsChange(parseInt(e.target.value))}
                  value={cOptions.fps}
                  type="number"
                  id="fps"
                  min={1}
                  max={120}
                />
              </div>
              <div className="flex flex-col gap-2">
                <h3 className="text-base font-bold">Audio</h3>
                <div className="flex items-center space-x-2">
                  <Checkbox
                    id="removeAudio"
                    checked={cOptions.removeAudio}
                    disabled={isDisabled}
                    onCheckedChange={(checked) => handleAudioChange(!!checked)}
                  />
                  <label
                    htmlFor="removeAudio"
                    className="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70"
                  >
                    Remove soundtrack
                  </label>
                </div>
              </div>
              <div className="flex flex-col gap-2">
                <h3 className="text-base font-bold">Preview</h3>
                <div className="flex items-center space-x-2">
                  <Checkbox
                    id="generatePreview"
                    disabled={isDisabled}
                    checked={cOptions.generatePreview}
                    onCheckedChange={(checked) => handlePreviewEnabledChange(!!checked)}
                  />
                  <label
                    htmlFor="generatePreview"
                    className="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70"
                  >
                    Generate preview automatically
                  </label>
                </div>
                <div className="flex flex-col gap-2">
                  <TooltipLabel
                    className="text-sm font-medium"
                    htmlFor="previewDuration"
                    tooltip="Duration of preview video for size estimation."
                  >
                    Preview Duration (seconds)
                  </TooltipLabel>
                  <Input
                    disabled={isDisabled}
                    onChange={(e) => handlePreviewDurationChange(parseInt(e.target.value))}
                    value={cOptions.previewDuration}
                    type="number"
                    min={1}
                    max={30}
                    id="previewDuration"
                  />
                </div>
              </div>
            </MotionTabsContent>
          )}
        </AnimatePresence>
      </Tabs>
    </TooltipProvider>
  )
}

interface ToggleItemProps {
  value: string
  icon: LucideIcon
  title: string
  description: string
}

const ToggleItem: React.FC<ToggleItemProps> = ({ value, icon: Icon, title, description }) => (
  <ToggleGroupItem
    variant="outline"
    className="flex flex-row w-full justify-start items-center gap-3 h-16"
    value={value}
    name={value}
    aria-label={`Toggle ${value}`}
  >
    <Icon className="h-7 w-7 flex-shrink-0" />
    <div className="flex flex-col text-left">
      <div className="text-sm font-semibold">{title}</div>
      <p className="text-xs">{description}</p>
    </div>
  </ToggleGroupItem>
)

interface TooltipLabelProps {
  htmlFor?: string
  className?: string
  children: React.ReactNode
  tooltip: string
}

const TooltipLabel: React.FC<TooltipLabelProps> = ({ htmlFor, className, children, tooltip }) => (
  <div className="flex items-center gap-2">
    <Label className={className} htmlFor={htmlFor}>
      {children}
    </Label>
    <Tooltip>
      <TooltipTrigger asChild>
        <InfoIcon className="size-4 text-muted-foreground" />
      </TooltipTrigger>
      <TooltipContent className="max-w-44">
        <p>{tooltip}</p>
      </TooltipContent>
    </Tooltip>
  </div>
)
