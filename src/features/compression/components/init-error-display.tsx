import { AlertCircle } from "lucide-react";

import { cn } from "@/lib/utils";

interface InitErrorDisplayProps {
  /** Error message from backend (e.g. initError from store). */
  message: string;
  className?: string;
}

function getTitleAndDescription(message: string): { title: string; description: string } {
  const lower = message.toLowerCase();
  if (lower.includes("ffmpeg not found")) {
    return {
      title: "FFmpeg Not Found",
      description: "You need to install FFmpeg first to get started",
    };
  }
  if (lower.includes("no supported") && lower.includes("codec")) {
    return {
      title: "No Supported Codecs",
      description: "Ensure FFmpeg is installed with codec support",
    };
  }
  const lines = message
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
  const firstLine = lines[0] ?? message;
  const title = firstLine.length > 60 ? `${firstLine.slice(0, 57)}…` : firstLine;
  const rest = lines.slice(1).join(" ").trim();
  return { title, description: rest };
}

export function InitErrorDisplay({ message, className }: InitErrorDisplayProps) {
  const { title, description } = getTitleAndDescription(message);

  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center gap-1 text-muted-foreground",
        className
      )}
    >
      <AlertCircle className={cn("mb-2 size-12")} />
      <p className={cn("text-sm font-medium")}>{title}</p>
      {description && <p className={cn("text-xs")}>{description}</p>}
    </div>
  );
}
