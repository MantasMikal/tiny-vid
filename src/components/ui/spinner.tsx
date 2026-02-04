import { LoaderPinwheel } from "lucide-react";

import { cn } from "@/lib/utils";

interface SpinnerProps {
  className?: string;
}

export function Spinner({ className }: SpinnerProps) {
  return (
    <div className={cn(`inline-block size-8 text-foreground`, className)} role="status">
      <LoaderPinwheel
        className={cn(
          "size-full animate-spin",
          "motion-reduce:animate-[spin_1.5s_linear_infinite]"
        )}
      />
      <span className={cn("sr-only")}>Loading...</span>
    </div>
  );
}
