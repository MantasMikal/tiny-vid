import { useShallow } from "zustand/react/shallow";

import { DropZone } from "@/components/ui/drop-zone";
import { selectIsInitialized } from "@/features/compression/store/compression-selectors";
import { useCompressionStore } from "@/features/compression/store/compression-store";
import { cn } from "@/lib/utils";
import { desktopClient } from "@/platform/desktop/client";

export function VideoDropZone() {
  const disabled = useCompressionStore(useShallow((s) => !selectIsInitialized(s)));

  return (
    <DropZone
      disabled={disabled}
      className="size-[calc(100%-8px)] rounded-sm border"
      onClick={() => void useCompressionStore.getState().browseAndSelectFile()}
      onDrop={(files) => {
        const firstFile = files[0] as (File & { path?: string }) | undefined;
        if (!firstFile) return;
        void (async () => {
          const resolvedPath = await desktopClient.pathForFile(firstFile);
          const fallbackPath = firstFile.path;
          const path = resolvedPath ?? (typeof fallbackPath === "string" ? fallbackPath : null);
          if (typeof path === "string" && path.length > 0) {
            await useCompressionStore.getState().selectPath(path);
          }
        })();
      }}
    >
      <p className={cn("text-center text-muted-foreground")}>Drop video or click to browse</p>
    </DropZone>
  );
}
