import { cn } from "@/lib/utils"

interface SpinnerProps {
  className?: string
}

export function Spinner({ className }: SpinnerProps) {
  return (
    <div
      className={cn(
        `
          inline-block size-8 animate-spin rounded-full border-4 border-solid
          border-current border-e-transparent align-[-0.125em] text-foreground
          motion-reduce:animate-[spin_1.5s_linear_infinite]
        `,
        className
      )}
      role="status"
    >
      <span className={cn("sr-only")}>Loading...</span>
    </div>
  )
}
