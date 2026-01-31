import { getCurrentWindow } from "@tauri-apps/api/window";
import { type ComponentPropsWithoutRef, useEffect, useRef } from "react";

import { cn } from "@/lib/utils";

export interface DropZoneProps extends Omit<
  ComponentPropsWithoutRef<"div">,
  "onDrop"
> {
  onDrop: (paths: string[]) => void;
  onClick?: () => void;
  disabled?: boolean;
}

export function DropZone({
  onDrop,
  onClick,
  disabled = false,
  className,
  children,
  ...props
}: DropZoneProps) {
  const ref = useRef<HTMLDivElement>(null);
  const onDropRef = useRef(onDrop);
  useEffect(() => {
    onDropRef.current = onDrop;
  }, [onDrop]);

  useEffect(() => {
    let mounted = true;
    const unlistenRef = { current: null as (() => void) | null };

    void getCurrentWindow()
      .onDragDropEvent((event) => {
        if (event.payload.type !== "drop" || event.payload.paths.length === 0) {
          return;
        }

        const rect = ref.current?.getBoundingClientRect();
        if (!rect) return;

        const scale = window.devicePixelRatio || 1;
        const x = event.payload.position.x / scale;
        const y = event.payload.position.y / scale;

        const isInside =
          x >= rect.left &&
          x <= rect.right &&
          y >= rect.top &&
          y <= rect.bottom;

        if (isInside) {
          onDropRef.current(event.payload.paths);
        }
      })
      .then((unlisten) => {
        if (mounted) {
          unlistenRef.current = unlisten;
        } else {
          unlisten();
        }
      })
      .catch(() => {
        /* ignore setup errors */
      });

    return () => {
      mounted = false;
      if (unlistenRef.current) {
        unlistenRef.current();
        unlistenRef.current = null;
      }
    };
  }, []);

  return (
    <div
      ref={ref}
      role="button"
      tabIndex={disabled ? -1 : 0}
      onClick={disabled ? undefined : onClick}
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
          flex size-full cursor-pointer flex-col items-center justify-center
          gap-4 rounded-md border-2 border-dashed border-muted-foreground/25
          transition-colors
          hover:border-primary/50
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
