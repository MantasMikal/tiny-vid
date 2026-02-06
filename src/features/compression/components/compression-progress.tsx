import { Progress } from "@/components/ui/progress";
import { Spinner } from "@/components/ui/spinner";
import { cn } from "@/lib/utils";

interface CompressionProgressProps {
  progress: number;
  progressStepLabel: string;
  className?: string;
}

export function CompressionProgress({
  progress,
  progressStepLabel,
  className,
}: CompressionProgressProps) {
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
