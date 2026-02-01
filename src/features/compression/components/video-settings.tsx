import {
  BikeIcon,
  CarFrontIcon,
  CookingPotIcon,
  InfoIcon,
  RocketIcon,
} from "lucide-react";
import { AnimatePresence, motion, usePresenceData } from "motion/react";
import { useEffect, useState } from "react";

import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { CompressionOptions } from "@/features/compression/lib/compression-options";
import {
  codecs,
  convertQualityForCodecSwitch,
  presets,
  qualityToCrf,
} from "@/features/compression/lib/compression-options";
import { useCompression } from "@/features/compression/store/use-compression";
import { cn } from "@/lib/utils";

type BasicPresets = "basic" | "super" | "ultra" | "cooked";
type TabOptions = "basic" | "advanced";

const toggleConfig = [
  {
    value: "basic",
    icon: BikeIcon,
    title: "Basic",
    description: "Basic compression with minimal loss in quality",
    options: {
      quality: 90,
      preset: "fast",
      fps: 30,
      scale: 1,
      removeAudio: false,
      codec: "libx264",
      generatePreview: true,
      tune: undefined,
    },
  },
  {
    value: "super",
    icon: CarFrontIcon,
    title: "Medium",
    description: "Medium compression with some loss in quality",
    options: {
      quality: 75,
      preset: "fast",
      fps: 30,
      scale: 1,
      removeAudio: false,
      codec: "libx264",
      generatePreview: true,
      tune: undefined,
    },
  },
  {
    value: "ultra",
    icon: RocketIcon,
    title: "Strong",
    description: "Strong compression with loss in quality",
    options: {
      quality: 60,
      preset: "fast",
      fps: 30,
      scale: 1,
      removeAudio: false,
      codec: "libx264",
      generatePreview: true,
      tune: undefined,
    },
  },
  {
    value: "cooked",
    icon: CookingPotIcon,
    title: "Cooked",
    description: "Deep fried with extra crunch",
    options: {
      quality: 40,
      preset: "fast",
      fps: 30,
      scale: 1,
      removeAudio: false,
      codec: "libx264",
      generatePreview: true,
      tune: undefined,
    },
  },
] as const;

const TAB_ORDER: TabOptions[] = ["basic", "advanced"];
const getTabIndex = (tab: TabOptions) => TAB_ORDER.indexOf(tab);

const MotionTabsContent = motion.create(TabsContent);

function AnimatedTabPanel({
  value,
  className,
  children,
}: {
  value: TabOptions;
  className?: string;
  children: React.ReactNode;
}) {
  const direction = usePresenceData() as number | undefined;
  const dir = direction ?? 0;

  return (
    <MotionTabsContent
      value={value}
      className={className}
      initial={{
        opacity: 0,
        transform: dir > 0 ? "translateX(100px)" : "translateX(-100px)",
      }}
      animate={{ opacity: 1, transform: "translateX(0)" }}
      exit={{
        opacity: 0,
        transform: dir > 0 ? "translateX(-100px)" : "translateX(100px)",
      }}
    >
      {children}
    </MotionTabsContent>
  );
}

interface VideoSettingsProps {
  isDisabled: boolean;
  cOptions: CompressionOptions;
  onOptionsChange: (
    options: CompressionOptions,
    opts?: { triggerPreview?: boolean }
  ) => void;
}

export function VideoSettings({
  isDisabled,
  cOptions,
  onOptionsChange,
}: VideoSettingsProps) {
  const [tabState, setTabState] = useState<{
    activeTab: TabOptions;
    direction: number;
  }>({ activeTab: "basic", direction: 0 });
  const { activeTab, direction } = tabState;
  const [basicPreset, setBasicPreset] = useState<BasicPresets>("super");
  const {
    ffmpegCommandPreview,
    ffmpegCommandPreviewLoading,
    refreshFfmpegCommandPreview,
  } = useCompression();

  useEffect(() => {
    if (activeTab === "advanced" && ffmpegCommandPreview === null) {
      void refreshFfmpegCommandPreview();
    }
  }, [activeTab, ffmpegCommandPreview, refreshFfmpegCommandPreview]);

  const handleTabChange = (value: string) => {
    const newTab = value as TabOptions;
    const prevIndex = getTabIndex(activeTab);
    const nextIndex = getTabIndex(newTab);
    const nextDirection = nextIndex > prevIndex ? 1 : -1;
    setTabState({ activeTab: newTab, direction: nextDirection });
  };

  return (
    <TooltipProvider>
      <Tabs
        value={activeTab}
        className={cn("w-full min-w-0")}
        onValueChange={handleTabChange}
      >
        <TabsList className={cn("mb-4 grid w-full grid-cols-2")}>
          <TabsTrigger value="basic">Basic</TabsTrigger>
          <TabsTrigger value="advanced">Advanced</TabsTrigger>
        </TabsList>
        <AnimatePresence initial={false} custom={direction}>
          {activeTab === "basic" && (
            <AnimatedTabPanel
              key="basic"
              value="basic"
              className={cn("flex flex-col gap-4")}
            >
              <div className={cn("flex flex-col gap-2")}>
                <h3 className={cn("text-base font-bold")}>Preset</h3>
                <ToggleGroup
                  value={basicPreset}
                  spacing={2}
                  onValueChange={(v) => {
                    if (!v) return;
                    const preset = toggleConfig.find((c) => c.value === v);
                    setBasicPreset(v as BasicPresets);
                    if (preset)
                      onOptionsChange({ ...cOptions, ...preset.options });
                  }}
                  disabled={isDisabled}
                  className={cn("w-full min-w-0 flex-col items-start")}
                  type="single"
                >
                  {toggleConfig.map((config) => (
                    <ToggleGroupItem
                      key={config.value}
                      variant="outline"
                      className={cn(
                        `
                          flex h-16 w-full min-w-0 flex-row items-center
                          justify-start gap-3 whitespace-normal
                        `
                      )}
                      value={config.value}
                    >
                      <config.icon className={cn("size-7 shrink-0")} />
                      <div
                        className={cn("flex min-w-0 flex-1 flex-col text-left")}
                      >
                        <div className={cn("text-sm font-semibold")}>
                          {config.title}
                        </div>
                        <p className={cn("text-xs wrap-break-word")}>
                          {config.description}
                        </p>
                      </div>
                    </ToggleGroupItem>
                  ))}
                </ToggleGroup>
              </div>
              <div className={cn("flex flex-col gap-2")}>
                <TooltipLabel tooltip="Omits all audio from output (FFmpeg -an). Saves space when you don't need sound; video-only encoding is faster.">
                  Audio
                </TooltipLabel>
                <div className={cn("flex items-center space-x-2")}>
                  <Checkbox
                    id="removeAudio"
                    disabled={isDisabled}
                    checked={cOptions.removeAudio}
                    onCheckedChange={(c) => {
                      onOptionsChange({ ...cOptions, removeAudio: !!c });
                    }}
                  />
                  <Label htmlFor="removeAudio">Remove soundtrack</Label>
                </div>
              </div>
            </AnimatedTabPanel>
          )}
          {activeTab === "advanced" && (
            <AnimatedTabPanel
              key="advanced"
              value="advanced"
              className={cn("flex flex-col gap-4")}
            >
              <div className={cn("flex flex-col gap-2")}>
                <TooltipLabel tooltip="Video encoder (-c:v). H.264: best compatibility. H.265: ~30–50% smaller files than H.264. AV1: best compression, royalty-free; may not play on older devices.">
                  Codec
                </TooltipLabel>
                <Select
                  value={cOptions.codec}
                  disabled={isDisabled}
                  onValueChange={(v) => {
                    const newQuality =
                      v !== cOptions.codec
                        ? convertQualityForCodecSwitch(
                            cOptions.quality,
                            cOptions.codec,
                            v
                          )
                        : cOptions.quality;
                    onOptionsChange({
                      ...cOptions,
                      codec: v,
                      quality: newQuality,
                    });
                  }}
                >
                  <SelectTrigger className={cn("w-full")}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {codecs.map((c) => (
                      <SelectItem key={c.value} value={c.value}>
                        {c.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className={cn("flex flex-col gap-2")}>
                <TooltipLabel tooltip="Constant Rate Factor (CRF): keeps perceived quality steady while varying bitrate. Lower = higher quality and larger files. A change of ±6 roughly halves or doubles file size. Mapped per codec (x264: 23–51, x265: 28–51, AV1: 24–63).">
                  Quality
                </TooltipLabel>
                <Slider
                  disabled={isDisabled}
                  min={0}
                  max={100}
                  step={1}
                  value={[cOptions.quality]}
                  showValueOnThumb
                  onValueChange={([v]) => {
                    onOptionsChange(
                      { ...cOptions, quality: v },
                      { triggerPreview: false }
                    );
                  }}
                  onValueCommit={([v]) => {
                    onOptionsChange({ ...cOptions, quality: v });
                  }}
                />
                <p className={cn("text-xs text-muted-foreground")}>
                  CRF {qualityToCrf(cOptions.quality, cOptions.codec)}
                </p>
              </div>
              <div className={cn("flex flex-col gap-2")}>
                <TooltipLabel tooltip="Encoding speed vs compression. Slower presets produce smaller files at the same quality but take longer to encode.">
                  Encoding Preset
                </TooltipLabel>
                <Select
                  value={cOptions.preset}
                  disabled={isDisabled}
                  onValueChange={(v) => {
                    onOptionsChange({
                      ...cOptions,
                      preset: v,
                    });
                  }}
                >
                  <SelectTrigger className={cn("w-full")}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {presets.map((p) => (
                      <SelectItem key={p.value} value={p.value}>
                        {p.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className={cn("flex flex-col gap-2")}>
                <TooltipLabel tooltip="Resize output (scale filter). 1.0 = original size. Lower values shrink resolution and file size; aspect ratio preserved, dimensions kept even for encoders.">
                  Resolution Scale
                </TooltipLabel>
                <Slider
                  disabled={isDisabled}
                  min={0.25}
                  max={1}
                  step={0.05}
                  value={[cOptions.scale]}
                  showValueOnThumb
                  formatThumbValue={(v) => `${String(Math.round(v * 100))}%`}
                  onValueChange={([v]) => {
                    onOptionsChange(
                      { ...cOptions, scale: v },
                      { triggerPreview: false }
                    );
                  }}
                  onValueCommit={([v]) => {
                    onOptionsChange({ ...cOptions, scale: v });
                  }}
                />
              </div>
              <div className={cn("flex flex-col gap-2")}>
                <TooltipLabel tooltip="Output frame rate (-r): target FPS. Encoder duplicates or drops frames to hit this rate. Common: 24 (film), 30 (NTSC), 60 (smooth). Lower values reduce file size.">
                  Frame Rate (FPS)
                </TooltipLabel>
                <Input
                  disabled={isDisabled}
                  type="number"
                  min={1}
                  max={120}
                  value={cOptions.fps}
                  onChange={(e) => {
                    onOptionsChange({
                      ...cOptions,
                      fps: parseInt(e.target.value) || 30,
                    });
                  }}
                />
              </div>
              <div className={cn("flex flex-col gap-2")}>
                <TooltipLabel tooltip="Creates a short preview clip at the start of the video using -t (duration). Lets you check quality quickly before compressing the full file.">
                  Preview
                </TooltipLabel>
                <div className={cn("flex items-center space-x-2")}>
                  <Checkbox
                    id="generatePreview"
                    disabled={isDisabled}
                    checked={cOptions.generatePreview ?? true}
                    onCheckedChange={(c) => {
                      onOptionsChange({
                        ...cOptions,
                        generatePreview: !!c,
                      });
                    }}
                  />
                  <Label htmlFor="generatePreview">
                    Generate preview automatically
                  </Label>
                </div>
                <Input
                  disabled={isDisabled}
                  type="number"
                  min={1}
                  max={30}
                  value={cOptions.previewDuration ?? 3}
                  onChange={(e) => {
                    onOptionsChange({
                      ...cOptions,
                      previewDuration: parseInt(e.target.value) || 3,
                    });
                  }}
                />
                <p className={cn("text-xs text-muted-foreground")}>
                  Duration in seconds (FFmpeg -t)
                </p>
              </div>
              <Accordion type="single" collapsible className={cn("w-full")}>
                <AccordionItem
                  value="ffmpeg-command"
                  className={cn("border-none")}
                >
                  <AccordionTrigger
                    className={cn(
                      "py-2 text-base font-bold",
                      "hover:no-underline"
                    )}
                  >
                    FFmpeg command
                  </AccordionTrigger>
                  <AccordionContent>
                    <ScrollArea
                      className={cn(
                        "rounded-md border bg-muted/50 p-3 font-mono text-xs",
                        "max-h-64 w-full overflow-auto break-all select-text"
                      )}
                    >
                      {ffmpegCommandPreviewLoading ? (
                        <p className={cn("text-muted-foreground")}>
                          Loading…
                        </p>
                      ) : ffmpegCommandPreview ? (
                        <pre className={cn("m-0 whitespace-pre-wrap select-text")}>
                          {ffmpegCommandPreview}
                        </pre>
                      ) : (
                        <p className={cn("text-muted-foreground")}>
                          Could not generate command
                        </p>
                      )}
                    </ScrollArea>
                  </AccordionContent>
                </AccordionItem>
              </Accordion>
            </AnimatedTabPanel>
          )}
        </AnimatePresence>
      </Tabs>
    </TooltipProvider>
  );
}

function TooltipLabel({
  children,
  tooltip,
}: {
  children: React.ReactNode;
  tooltip: string;
}) {
  return (
    <div className={cn("flex items-center gap-2")}>
      <Label className={cn("text-base font-bold")}>{children}</Label>
      <Tooltip>
        <TooltipTrigger asChild>
          <button type="button">
            <InfoIcon className={cn("size-4 text-muted-foreground")} />
          </button>
        </TooltipTrigger>
        <TooltipContent className={cn("max-w-72")}>
          <p>{tooltip}</p>
        </TooltipContent>
      </Tooltip>
    </div>
  );
}
