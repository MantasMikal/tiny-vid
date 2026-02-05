import { type ComponentPropsWithoutRef } from "react";

import { cn } from "@/lib/utils";

export interface DropZoneProps extends Omit<ComponentPropsWithoutRef<"div">, "onDrop"> {
  onDrop?: (files: File[]) => void;
  onClick?: () => void;
  disabled?: boolean;
}

export function DropZone({
  onDrop,
  onClick,
  disabled = false,
  className,
  children,
  ref,
  ...props
}: DropZoneProps & { ref?: React.Ref<HTMLDivElement> }) {
  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    const files = e.dataTransfer.files;
    if (files.length > 0) {
      onDrop?.(Array.from(files));
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onClick?.();
    }
  };

  return (
    <div
      ref={ref}
      role="button"
      tabIndex={disabled ? -1 : 0}
      onClick={disabled ? undefined : onClick}
      onDragOver={onDrop && !disabled ? handleDragOver : undefined}
      onDrop={onDrop && !disabled ? handleDrop : undefined}
      onKeyDown={disabled ? undefined : handleKeyDown}
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
}
