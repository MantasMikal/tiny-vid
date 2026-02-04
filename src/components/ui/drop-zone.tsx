import { type ComponentPropsWithoutRef, forwardRef } from "react";

import { cn } from "@/lib/utils";

export interface DropZoneProps extends Omit<ComponentPropsWithoutRef<"div">, "onDrop"> {
  onDrop?: (files: File[]) => void;
  onClick?: () => void;
  disabled?: boolean;
}

export const DropZone = forwardRef<HTMLDivElement, DropZoneProps>(function DropZone(
  { onDrop, onClick, disabled = false, className, children, ...props },
  ref
) {
  return (
    <div
      ref={ref}
      role="button"
      tabIndex={disabled ? -1 : 0}
      onClick={disabled ? undefined : onClick}
      onDragOver={
        onDrop && !disabled
          ? (e) => {
              e.preventDefault();
              e.dataTransfer.dropEffect = "copy";
            }
          : undefined
      }
      onDrop={
        onDrop && !disabled
          ? (e) => {
              e.preventDefault();
              const files = e.dataTransfer.files;
              if (files.length > 0) {
                onDrop(Array.from(files));
              }
            }
          : undefined
      }
      onKeyDown={
        disabled
          ? undefined
          : (e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                onClick?.();
              }
            }
      }
      className={cn(
        `
          flex size-full cursor-pointer flex-col items-center justify-center gap-4 rounded-md border-2 border-dashed
          border-muted-foreground/25 ring-offset-background transition-colors
          hover:border-primary/50
          focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:outline-none
        `,
        disabled && "pointer-events-none opacity-50",
        className
      )}
      {...props}
    >
      {children}
    </div>
  );
});
