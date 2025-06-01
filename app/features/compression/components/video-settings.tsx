"use client";

import { motion, AnimatePresence } from "framer-motion";
import { Label } from "@/app/components/ui/label";
import { Slider } from "@/app/components/ui/slider";
import { Input } from "@/app/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/app/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/app/components/ui/tabs";
import { ToggleGroup, ToggleGroupItem } from "@/app/components/ui/toggle-group";
import {
  BikeIcon,
  CarFrontIcon,
  CookingPotIcon,
  LucideIcon,
  RocketIcon,
} from "lucide-react";
import { useState } from "react";
import { Checkbox } from "@/app/components/ui/checkbox";

export type CompressionOptions = {
  quality: number;
  preset: (typeof presets)[number]["value"];
  fps: number;
  scale: number;
  removeAudio: boolean;
  generatePreview?: boolean;
  previewDuration?: number;
};

type BasicPresets = "basic" | "super" | "ultra" | "cooked";
type TabOptions = "basic" | "advanced";

type ConfigOption = {
  value: string;
  icon: LucideIcon;
  title: string;
  description: string;
  options: CompressionOptions;
};

const toggleConfig: ConfigOption[] = [
  {
    value: "basic",
    icon: BikeIcon,
    title: "Basic",
    description: "Basic compression with minimal loss in quality",
    options: {
      quality: 90,
      preset: "superfast",
      fps: 30,
      scale: 1,
      removeAudio: false,
      generatePreview: true,
    },
  },
  {
    value: "super",
    icon: CarFrontIcon,
    title: "Medium",
    description: "Medium compression with some loss in quality",
    options: {
      quality: 65,
      preset: "veryfast",
      fps: 30,
      scale: 1,
      removeAudio: false,
      generatePreview: true,
    },
  },
  {
    value: "ultra",
    icon: RocketIcon,
    title: "Strong",
    description: "Strong compression with loss in quality",
    options: {
      quality: 50,
      preset: "superfast",
      fps: 30,
      scale: 1,
      removeAudio: false,
      generatePreview: true,
    },
  },
  {
    value: "cooked",
    icon: CookingPotIcon,
    title: "Cooked",
    description: "Deep fried with extra crunch",
    options: {
      quality: 30,
      preset: "superfast",
      fps: 30,
      scale: 1,
      removeAudio: false,
      generatePreview: true,
    },
  },
] as const;

export const presets = [
  {
    name: "Ultra Fast",
    value: "ultrafast",
  },
  {
    name: "Super Fast",
    value: "superfast",
  },
  {
    name: "Very Fast",
    value: "veryfast",
  },
  {
    name: "Faster",
    value: "faster",
  },
  {
    name: "Fast",
    value: "fast",
  },
  {
    name: "Medium",
    value: "medium",
  },
  {
    name: "Slow",
    value: "slow",
  },
] as const;

const MotionTabsContent = motion.create(TabsContent);

interface VideoSettingsProps {
  isDisabled: boolean;
  cOptions: CompressionOptions;
  onOptionsChange: (options: CompressionOptions) => void;
}

export function VideoSettings({
  isDisabled,
  cOptions,
  onOptionsChange,
}: VideoSettingsProps) {
  const [activeTab, setActiveTab] = useState<TabOptions>("basic");
  const [basicPreset, setBasicPreset] = useState<BasicPresets>("super");

  const handleQualityChange = (value: number) => {
    onOptionsChange({
      ...cOptions,
      quality: value,
    });
  };

  const handleScaleChange = (value: number) => {
    onOptionsChange({
      ...cOptions,
      scale: value,
    });
  };

  const handlePresetChange = (value: string) => {
    onOptionsChange({
      ...cOptions,
      preset: value as CompressionOptions["preset"],
    });
  };

  const handleFpsChange = (value: number | string) => {
    if (typeof value === "number") {
      onOptionsChange({
        ...cOptions,
        fps: value,
      });
    }
  };

  const handleAudioChange = (value: boolean) => {
    onOptionsChange({
      ...cOptions,
      removeAudio: value,
    });
  };

  const handlePreviewDurationChange = (value: number) => {
    onOptionsChange({
      ...cOptions,
      previewDuration: value,
    });
  };

  const handlePreviewEnabledChange = (value: boolean) => {
    onOptionsChange({
      ...cOptions,
      generatePreview: value,
    });
  };

  const handleBasicPresetChange = (value: BasicPresets) => {
    if (!value) return;
    const preset = toggleConfig.find((config) => config.value === value);
    setBasicPreset(value);
    if (preset) {
      onOptionsChange({
        ...cOptions,
        ...preset.options,
      });
    }
  };

  return (
    <Tabs
      value={activeTab}
      className="w-full"
      onValueChange={(value) => setActiveTab(value as TabOptions)}
    >
      <TabsList className="grid w-full grid-cols-2">
        <TabsTrigger value="basic">Basic</TabsTrigger>
        <TabsTrigger value="advanced">Advanced</TabsTrigger>
      </TabsList>
      <AnimatePresence initial={false}>
        {activeTab === "basic" && (
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
        {activeTab === "advanced" && (
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
              <Label className="text-base font-bold" htmlFor="quality">
                Quality
              </Label>
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
                  handleQualityChange(value[0]);
                }}
              />
              <p className="text-sm text-gray-500">
                Lower quality will result in smaller file size. At maximum
                quality the video will still be compressed with minimum impact
                on quality.
              </p>
            </div>
            <div className="flex flex-col gap-2">
              <Label className="text-base font-bold" htmlFor="scale">
                Scale
              </Label>
              <Slider
                disabled={isDisabled}
                name="scale"
                id="scale"
                min={0.01}
                max={1}
                step={0.01}
                defaultValue={[cOptions.scale]}
                value={[cOptions.scale]}
                onValueChange={(value) => handleScaleChange(value[0])}
              />
              <p className="text-sm text-gray-500">
                This will shrink the video resolution. Can greatly reduce file
                size.
              </p>
            </div>
            <div className="flex flex-col gap-2">
              <div className="flex flex-col gap-2">
                <Label className="text-base font-bold" htmlFor="preset">
                  Preset
                </Label>
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
              <p className="text-sm text-gray-500">
                Compression speed. A slower preset will provide slightly better
                quality, but will take longer to process and potential crash the compression. Faster values are
                recommended for most cases.
              </p>
            </div>
            <div className="flex flex-col gap-2">
              <Label className="text-base font-bold" htmlFor="fps">
                FPS
              </Label>
              <Input
                disabled={isDisabled}
                onChange={(e) => handleFpsChange(parseInt(e.target.value))}
                value={cOptions.fps}
                type="number"
                id="fps"
                max={120}
              />
              <p className="text-sm text-gray-500">
                Frames per second. Lower FPS will result in smaller file size
              </p>
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
                  onCheckedChange={(checked) =>
                    handlePreviewEnabledChange(!!checked)
                  }
                />
                <label
                  htmlFor="generatePreview"
                  className="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70"
                >
                  Automatically Generate previews
                </label>
              </div>
            </div>
            <div className="flex flex-col gap-2">
              <Label className="text-base font-bold" htmlFor="previewDuration">
                Preview Duration (seconds)
              </Label>
              <Input
                disabled={isDisabled}
                onChange={(e) =>
                  handlePreviewDurationChange(parseInt(e.target.value))
                }
                value={cOptions.previewDuration}
                type="number"
                min={1}
                id="previewDuration"
              />
              <p className="text-sm text-gray-500">
                Will change the duration of the preview video. Will provide
                better estimate of the output file size.
              </p>
            </div>
          </MotionTabsContent>
        )}
      </AnimatePresence>
    </Tabs>
  );
}

interface ToggleItemProps {
  value: string;
  icon: LucideIcon;
  title: string;
  description: string;
}

const ToggleItem: React.FC<ToggleItemProps> = ({
  value,
  icon: Icon,
  title,
  description,
}) => (
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
);
