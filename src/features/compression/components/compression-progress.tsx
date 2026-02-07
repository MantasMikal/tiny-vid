import { useShallow } from "zustand/react/shallow";

import { Progress } from "@/components/ui/progress";
import { Spinner } from "@/components/ui/spinner";
import { getProgressStepLabel } from "@/features/compression/lib/preview-progress";
import { useCompressionStore } from "@/features/compression/store/compression-store";
import { cn } from "@/lib/utils";

interface CompressionProgressProps {
  className?: string;
}

export function CompressionProgress({ className }: CompressionProgressProps) {
  const { progress, progressStep } = useCompressionStore(
    useShallow((s) => ({ progress: s.progress, progressStep: s.progressStep }))
  );
  const progressStepLabel = getProgressStepLabel(progressStep);
  return (
    <div
      className={cn("flex flex-col gap-1 rounded-md border bg-background p-2 px-3 py-2", className)}
    >
      <span className={cn("flex items-center gap-1 text-xs text-foreground/90")}>
        <Spinner className={cn("size-3")} />
        {progressStepLabel}
      </span>
      <Progress className={cn("h-1.5 w-full")} value={progress * 100} />
    </div>
  );
}
