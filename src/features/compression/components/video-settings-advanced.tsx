import { InfoIcon } from "lucide-react";
import { useRef } from "react";

import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import { Checkbox } from "@/components/ui/checkbox";
import { ClampedNumberInput } from "@/components/ui/clamped-number-input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { CompressionOptions } from "@/features/compression/lib/compression-options";
import {
  audioBitratePresets,
  getAvailableFormats,
  getCodecInfo,
  getCodecsForFormat,
  getFormatCapabilities,
  isCodec,
  isFormat,
  isPresetValue,
  presets,
  supportsDownmixOption,
  tuneOptions,
} from "@/features/compression/lib/compression-options";
import type { VideoMetadata } from "@/features/compression/lib/get-video-metadata";
import { resolve } from "@/features/compression/lib/options-pipeline";
import { cn } from "@/lib/utils";
import type { CodecInfo } from "@/types/native";

type SetOptionsFn = (options: CompressionOptions, opts?: { triggerPreview?: boolean }) => void;

interface VideoSettingsAdvancedProps {
  cOptions: CompressionOptions;
  setOptions: SetOptionsFn;
  availableCodecs: CodecInfo[];
  isDisabled: boolean;
  videoMetadata: VideoMetadata | null | undefined;
  ffmpegCommandPreview: string | null;
}

function LabeledControl({
  label,
  tooltip,
  children,
}: {
  label: string;
  tooltip: string;
  children: React.ReactNode;
}) {
  return (
    <div className={cn("flex flex-col gap-2")}>
      <div className={cn("flex items-center gap-2")}>
        <Label className={cn("font-bold")}>{label}</Label>
        <Tooltip>
          <TooltipTrigger>
            <InfoIcon className={cn("size-4 text-muted-foreground")} />
          </TooltipTrigger>
          <TooltipContent
            align="start"
            avoidCollisions
            sideOffset={4}
            className={cn("max-w-[255px]")}
          >
            <p>{tooltip}</p>
          </TooltipContent>
        </Tooltip>
      </div>
      {children}
    </div>
  );
}

function CheckboxWithTooltip({
  id,
  label,
  tooltip,
  checked,
  onCheckedChange,
  disabled,
}: {
  id: string;
  label: string;
  tooltip: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <div className={cn("flex flex-col gap-2")}>
      <Tooltip>
        <TooltipTrigger asChild>
          <div className={cn("flex items-center space-x-2")}>
            <Checkbox
              id={id}
              disabled={disabled}
              checked={checked}
              onCheckedChange={(c) => onCheckedChange(!!c)}
            />
            <Label htmlFor={id}>{label}</Label>
          </div>
        </TooltipTrigger>
        <TooltipContent
          align="start"
          avoidCollisions
          sideOffset={4}
          className={cn("max-w-[255px]")}
        >
          <p>{tooltip}</p>
        </TooltipContent>
      </Tooltip>
    </div>
  );
}

function InputGroup({
  title,
  value,
  children,
}: {
  title: string;
  value: string;
  children: React.ReactNode;
}) {
  return (
    <AccordionItem value={value} className={cn("border-b last:border-b-0")}>
      <AccordionTrigger className={cn("py-2 text-base font-bold")}>{title}</AccordionTrigger>
      <AccordionContent className={cn("flex min-w-0 flex-col gap-4 pt-2")}>
        {children}
      </AccordionContent>
    </AccordionItem>
  );
}

export function VideoSettingsAdvanced({
  cOptions,
  setOptions,
  availableCodecs,
  isDisabled,
  videoMetadata,
  ffmpegCommandPreview,
}: VideoSettingsAdvancedProps) {
  const ffmpegAccordionRef = useRef<HTMLDivElement>(null);
  const availableFormats = getAvailableFormats(availableCodecs);
  const currentCodec = getCodecInfo(cOptions.codec, availableCodecs);
  const hasNoAudio = (videoMetadata?.audioStreamCount ?? 0) === 0;
  const isAlreadyStereo = (videoMetadata?.audioChannels ?? 0) <= 2;

  const handleAccordionChange = (value: string) => {
    if (value === "ffmpeg-command") {
      setTimeout(() => {
        ffmpegAccordionRef.current?.scrollIntoView({
          behavior: "smooth",
          block: "end",
        });
      }, 220);
    }
  };

  return (
    <Accordion
      type="single"
      collapsible
      className={cn("w-full min-w-0")}
      onValueChange={handleAccordionChange}
      defaultValue="output"
    >
      <InputGroup title="Output" value="output">
        <LabeledControl
          label="Format"
          tooltip="Container format: the file wrapper that holds video and audio streams."
        >
          <Select
            value={cOptions.outputFormat}
            disabled={isDisabled}
            onValueChange={(v) => {
              if (!isFormat(v)) return;
              setOptions(resolve({ ...cOptions, outputFormat: v }, availableCodecs), {
                triggerPreview: false,
              });
            }}
          >
            <SelectTrigger className={cn("w-full")}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {availableFormats.map((format) => {
                const name =
                  (isFormat(format) ? getFormatCapabilities(format).name : null) ??
                  format.toUpperCase();
                return (
                  <SelectItem key={format} value={format}>
                    {name}
                  </SelectItem>
                );
              })}
            </SelectContent>
          </Select>
        </LabeledControl>
        <LabeledControl
          label="Codec"
          tooltip="Video codec: the algorithm that compresses the video. Different codecs offer different compression and compatibility."
        >
          <Select
            value={cOptions.codec}
            disabled={isDisabled}
            onValueChange={(v) => {
              if (!isCodec(v, availableCodecs)) return;
              setOptions(resolve({ ...cOptions, codec: v }, availableCodecs));
            }}
          >
            <SelectTrigger className={cn("w-full")}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {getCodecsForFormat(cOptions.outputFormat, availableCodecs).map((codec) => (
                <SelectItem key={codec.value} value={codec.value}>
                  {codec.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </LabeledControl>
        <LabeledControl
          label="Quality"
          tooltip="Quality vs file size. Higher = better quality, larger file."
        >
          <Slider
            disabled={isDisabled}
            min={1}
            max={100}
            step={1}
            value={[cOptions.quality]}
            showValueOnThumb
            onValueChange={([v]) => {
              setOptions({ ...cOptions, quality: v }, { triggerPreview: false });
            }}
            onValueCommit={([v]) => {
              setOptions({ ...cOptions, quality: v });
            }}
          />
        </LabeledControl>
        {currentCodec?.presetType !== "vt" && (
          <LabeledControl
            label="Encoding Preset"
            tooltip="Encoding speed vs compression. Slower presets produce smaller files at the same quality but take longer to encode."
          >
            <Select
              value={cOptions.preset}
              disabled={isDisabled}
              onValueChange={(v) => {
                if (!isPresetValue(v)) return;
                setOptions({ ...cOptions, preset: v });
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
          </LabeledControl>
        )}
        {currentCodec?.supportsTune && (
          <LabeledControl
            label="Tune"
            tooltip="x264 tune: optimizes for specific content (film, animation, etc.). Only applies to H.264."
          >
            <Select
              value={cOptions.tune ?? "none"}
              disabled={isDisabled}
              onValueChange={(v) => {
                setOptions({
                  ...cOptions,
                  tune: v === "none" ? undefined : v,
                });
              }}
            >
              <SelectTrigger className={cn("w-full")}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {tuneOptions.map((t) => (
                  <SelectItem key={t.value} value={t.value}>
                    {t.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </LabeledControl>
        )}
        <LabeledControl
          label="Resolution Scale"
          tooltip="Resize output (scale filter). 1.0 = original size. Lower values shrink resolution and file size; aspect ratio preserved, dimensions kept even for encoders."
        >
          <Slider
            disabled={isDisabled}
            min={0.25}
            max={1}
            step={0.05}
            value={[cOptions.scale]}
            showValueOnThumb
            formatThumbValue={(v) => `${String(Math.round(v * 100))}%`}
            onValueChange={([v]) => {
              setOptions({ ...cOptions, scale: v }, { triggerPreview: false });
            }}
            onValueCommit={([v]) => {
              setOptions({ ...cOptions, scale: v });
            }}
          />
        </LabeledControl>
        <LabeledControl
          label="Frame Rate (FPS)"
          tooltip="Output frame rate (-r): target FPS. Encoder duplicates or drops frames to hit this rate. Common: 24 (film), 30 (NTSC), 60 (smooth). Lower values reduce file size."
        >
          <ClampedNumberInput
            disabled={isDisabled}
            min={1}
            max={120}
            value={cOptions.fps}
            onChange={(fps) => {
              setOptions({ ...cOptions, fps });
            }}
          />
        </LabeledControl>
      </InputGroup>
      <InputGroup title="Audio" value="audio">
        <CheckboxWithTooltip
          id="removeAudio"
          label="Remove Audio"
          tooltip={
            hasNoAudio
              ? "No audio in source"
              : "Omits all audio from output (FFmpeg -an). Saves space when you don't need sound; video-only encoding is faster."
          }
          checked={cOptions.removeAudio}
          onCheckedChange={(c) => setOptions({ ...cOptions, removeAudio: c })}
          disabled={isDisabled || hasNoAudio}
        />
        {(videoMetadata?.audioStreamCount ?? 0) > 1 && (
          <CheckboxWithTooltip
            id="preserveAdditionalAudioStreams"
            label="Preserve additional audio streams"
            tooltip={
              cOptions.removeAudio
                ? "Enable audio to preserve additional streams"
                : cOptions.outputFormat === "webm"
                  ? "WebM supports a single audio stream"
                  : "Include all audio streams in the output (transcoded to AAC/Opus). Only the first stream is used for preview."
            }
            checked={cOptions.preserveAdditionalAudioStreams ?? false}
            onCheckedChange={(c) => setOptions({ ...cOptions, preserveAdditionalAudioStreams: c })}
            disabled={isDisabled || cOptions.outputFormat === "webm" || cOptions.removeAudio}
          />
        )}
        <LabeledControl
          label="Audio Bitrate"
          tooltip={
            cOptions.removeAudio
              ? "Enable audio to configure bitrate"
              : "Audio bitrate in kbps. Higher values improve quality for multichannel (5.1, 7.1)."
          }
        >
          <Select
            value={String(cOptions.audioBitrate ?? 128)}
            disabled={isDisabled || hasNoAudio || cOptions.removeAudio}
            onValueChange={(v) => {
              const n = Number.parseInt(v, 10);
              if (Number.isFinite(n)) {
                setOptions({ ...cOptions, audioBitrate: n });
              }
            }}
          >
            <SelectTrigger className={cn("w-full")}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {audioBitratePresets.map((p) => (
                <SelectItem key={p.value} value={String(p.value)}>
                  {p.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          {(videoMetadata?.audioChannels ?? 0) > 2 && !cOptions.removeAudio && (
            <p className={cn("text-xs text-muted-foreground")}>
              Consider 192+ for 5.1, 256+ for 7.1
            </p>
          )}
        </LabeledControl>
        {supportsDownmixOption(cOptions.outputFormat, cOptions.codec) && (
          <CheckboxWithTooltip
            id="downmixToStereo"
            label="Downmix to stereo"
            tooltip={
              cOptions.removeAudio
                ? "Enable audio to configure downmix"
                : isAlreadyStereo
                  ? "Source is already stereo"
                  : "Convert multichannel (5.1, 7.1) to stereo. Saves space; useful for headphones or stereo playback."
            }
            checked={cOptions.downmixToStereo ?? false}
            onCheckedChange={(c) => setOptions({ ...cOptions, downmixToStereo: c })}
            disabled={isDisabled || isAlreadyStereo || cOptions.removeAudio}
          />
        )}
      </InputGroup>
      <InputGroup title="Metadata & streams" value="metadata">
        <CheckboxWithTooltip
          id="preserveMetadata"
          label="Preserve metadata"
          tooltip="Copy input metadata (title, creation date, etc.) to output file."
          checked={cOptions.preserveMetadata ?? false}
          onCheckedChange={(c) => setOptions({ ...cOptions, preserveMetadata: c })}
          disabled={isDisabled}
        />
        {(videoMetadata?.subtitleStreamCount ?? 0) > 0 && (
          <CheckboxWithTooltip
            id="preserveSubtitles"
            label="Preserve subtitles"
            tooltip="Include all subtitle streams in the output."
            checked={cOptions.preserveSubtitles ?? false}
            onCheckedChange={(c) => setOptions({ ...cOptions, preserveSubtitles: c })}
            disabled={isDisabled}
          />
        )}
      </InputGroup>
      <InputGroup title="Preview" value="preview">
        <CheckboxWithTooltip
          id="generatePreview"
          label="Generate preview automatically"
          tooltip="Creates a short preview clip at the start of the video using -t (duration). Lets you check quality quickly before compressing the full file."
          checked={cOptions.generatePreview ?? true}
          onCheckedChange={(c) => setOptions({ ...cOptions, generatePreview: c })}
          disabled={isDisabled}
        />
        <LabeledControl
          label="Duration (seconds)"
          tooltip="Duration of the preview clip in seconds (FFmpeg -t)."
        >
          <ClampedNumberInput
            disabled={isDisabled}
            min={1}
            max={30}
            value={cOptions.previewDuration ?? 3}
            onChange={(clamped) => {
              setOptions({
                ...cOptions,
                previewDuration: clamped,
              });
            }}
          />
        </LabeledControl>
      </InputGroup>
      <InputGroup title="FFmpeg command" value="ffmpeg-command">
        <div
          ref={ffmpegAccordionRef}
          className="max-w-[256px] overflow-x-auto rounded-md border bg-muted/50 p-3 font-mono text-xs select-text"
        >
          {ffmpegCommandPreview ? (
            <pre className={cn("m-0 max-w-xs whitespace-pre select-text")}>
              {ffmpegCommandPreview}
            </pre>
          ) : (
            <p className={cn("text-muted-foreground")}>Could not generate command</p>
          )}
        </div>
      </InputGroup>
    </Accordion>
  );
}
