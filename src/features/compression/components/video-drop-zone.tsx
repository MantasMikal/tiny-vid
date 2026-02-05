import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef } from "react";
import { useShallow } from "zustand/react/shallow";

import { DropZone } from "@/components/ui/drop-zone";
import { selectIsInitialized } from "@/features/compression/store/compression-selectors";
import { useCompressionStore } from "@/features/compression/store/compression-store";
import { cn } from "@/lib/utils";

export function VideoDropZone() {
  const ref = useRef<HTMLDivElement>(null);
  const disabled = useCompressionStore(useShallow((s) => !selectIsInitialized(s)));

  useEffect(() => {
    let mounted = true;
    const unlistenRef = { current: null as (() => void) | null };

    void getCurrentWindow()
      .onDragDropEvent((event) => {
        if (!selectIsInitialized(useCompressionStore.getState())) {
          return;
        }
        if (event.payload.type !== "drop" || event.payload.paths.length === 0) {
          return;
        }

        const rect = ref.current?.getBoundingClientRect();
        if (!rect) return;

        const scale = window.devicePixelRatio || 1;
        const x = event.payload.position.x / scale;
        const y = event.payload.position.y / scale;

        const isInside = x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;

        if (isInside) {
          const path = event.payload.paths[0];
          if (typeof path === "string") {
            void useCompressionStore.getState().selectPath(path);
          }
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
        console.error("Failed to setup drag drop event listener");
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
    <DropZone
      ref={ref}
      disabled={disabled}
      onClick={() => void useCompressionStore.getState().browseAndSelectFile()}
    >
      <p className={cn("text-center text-muted-foreground")}>Drop video or click to browse</p>
    </DropZone>
  );
}
