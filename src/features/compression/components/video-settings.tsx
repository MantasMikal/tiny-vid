import { BikeIcon, CarFrontIcon, CookingPotIcon, RocketIcon } from "lucide-react";
import { AnimatePresence, motion, usePresenceData } from "motion/react";
import { useEffect, useState } from "react";
import { useShallow } from "zustand/react/shallow";

import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { VideoSettingsAdvanced } from "@/features/compression/components/video-settings-advanced";
import {
  applyPreset,
  type BasicPresetId,
  DEFAULT_PRESET_ID,
  isBasicPresetId,
} from "@/features/compression/lib/options-pipeline";
import { selectIsActionsDisabled } from "@/features/compression/store/compression-selectors";
import {
  getCompressionState,
  useCompressionStore,
} from "@/features/compression/store/compression-store";
import { cn } from "@/lib/utils";

type TabOptions = "basic" | "advanced";

const TOGGLE_CONFIG: {
  value: BasicPresetId;
  icon: typeof BikeIcon;
  title: string;
  description: string;
}[] = [
  {
    value: "basic",
    icon: BikeIcon,
    title: "Basic",
    description: "Basic compression with minimal loss in quality",
  },
  {
    value: "super",
    icon: CarFrontIcon,
    title: "Medium",
    description: "Medium compression with some loss in quality",
  },
  {
    value: "ultra",
    icon: RocketIcon,
    title: "Strong",
    description: "Strong compression with loss in quality",
  },
  {
    value: "cooked",
    icon: CookingPotIcon,
    title: "Cooked",
    description: "Deep fried with extra crunch",
  },
];

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

export function VideoSettings() {
  const [tabState, setTabState] = useState<{
    activeTab: TabOptions;
    direction: number;
  }>({ activeTab: "basic", direction: 0 });
  const { activeTab, direction } = tabState;
  const [basicPreset, setBasicPreset] = useState<BasicPresetId>(DEFAULT_PRESET_ID);
  const {
    compressionOptions: cOptions,
    availableCodecs,
    isDisabled,
    ffmpegCommandPreview,
    videoMetadata,
  } = useCompressionStore(
    useShallow((s) => ({
      compressionOptions: s.compressionOptions,
      availableCodecs: s.availableCodecs,
      isDisabled: selectIsActionsDisabled(s),
      ffmpegCommandPreview: s.ffmpegCommandPreview,
      videoMetadata: s.videoMetadata,
    }))
  );

  const setOptions = getCompressionState().setCompressionOptions;

  useEffect(() => {
    if (activeTab === "advanced" && ffmpegCommandPreview === null) {
      void getCompressionState().refreshFfmpegCommandPreview();
    }
  }, [activeTab, ffmpegCommandPreview]);

  const handleTabChange = (value: string) => {
    const newTab: TabOptions = value === "advanced" ? "advanced" : "basic";
    const prevIndex = getTabIndex(activeTab);
    const nextIndex = getTabIndex(newTab);
    const nextDirection = nextIndex > prevIndex ? 1 : -1;
    setTabState({ activeTab: newTab, direction: nextDirection });
  };

  if (!cOptions) return null;
  console.log("SETTINGS RENDERED");

  return (
    <TooltipProvider>
      <Tabs value={activeTab} className={cn("w-full min-w-0")} onValueChange={handleTabChange}>
        <TabsList className={cn("mb-4 grid w-full grid-cols-2")}>
          <TabsTrigger value="basic">Basic</TabsTrigger>
          <TabsTrigger value="advanced">Advanced</TabsTrigger>
        </TabsList>
        <AnimatePresence initial={false} custom={direction}>
          {activeTab === "basic" && (
            <AnimatedTabPanel key="basic" value="basic" className={cn("flex flex-col gap-4")}>
              <div className={cn("flex flex-col gap-2")}>
                <h3 className={cn("text-base font-bold")}>Preset</h3>
                <ToggleGroup
                  value={basicPreset}
                  spacing={2}
                  onValueChange={(v) => {
                    if (!v || !isBasicPresetId(v)) return;
                    setBasicPreset(v);
                    setOptions(applyPreset(cOptions, v, availableCodecs));
                  }}
                  disabled={isDisabled}
                  className={cn("w-full min-w-0 flex-col items-start")}
                  type="single"
                >
                  {TOGGLE_CONFIG.map((config) => (
                    <ToggleGroupItem
                      key={config.value}
                      variant="outline"
                      className="flex h-16 w-full min-w-0 flex-row items-center justify-start gap-3 text-left"
                      value={config.value}
                    >
                      <config.icon className={cn("size-7 shrink-0")} />
                      <div className={cn("flex min-w-0 flex-1 flex-col whitespace-normal")}>
                        <div className={cn("text-sm font-semibold")}>{config.title}</div>
                        <p className={cn("text-xs wrap-break-word")}>{config.description}</p>
                      </div>
                    </ToggleGroupItem>
                  ))}
                </ToggleGroup>
              </div>
              <div className={cn("flex flex-col gap-2")}>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <div className={cn("flex items-center space-x-2")}>
                      <Checkbox
                        id="removeAudio-basic"
                        disabled={isDisabled}
                        checked={cOptions.removeAudio}
                        onCheckedChange={(c) => {
                          setOptions({ ...cOptions, removeAudio: !!c });
                        }}
                      />
                      <Label htmlFor="removeAudio-basic">Remove Audio</Label>
                    </div>
                  </TooltipTrigger>
                  <TooltipContent className={cn("max-w-72")}>
                    <p>
                      Omits all audio from output (FFmpeg -an). Saves space when you don&apos;t need
                      sound; video-only encoding is faster.
                    </p>
                  </TooltipContent>
                </Tooltip>
              </div>
            </AnimatedTabPanel>
          )}
          {activeTab === "advanced" && (
            <AnimatedTabPanel
              key="advanced"
              value="advanced"
              className={cn("flex min-w-0 flex-col gap-4")}
            >
              <VideoSettingsAdvanced
                cOptions={cOptions}
                setOptions={setOptions}
                availableCodecs={availableCodecs}
                isDisabled={isDisabled}
                videoMetadata={videoMetadata}
                ffmpegCommandPreview={ffmpegCommandPreview}
              />
            </AnimatedTabPanel>
          )}
        </AnimatePresence>
      </Tabs>
    </TooltipProvider>
  );
}
