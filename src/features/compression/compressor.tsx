import { ScrollArea } from "@/components/ui/scroll-area";
import { CompressionDetailsCard } from "@/features/compression/components/compression-details-card";
import { VideoSettings } from "@/features/compression/components/video-settings";
import { VideoWorkspace } from "@/features/compression/components/video-workspace";
import { cn } from "@/lib/utils";

export default function Compressor() {
  return (
    <div
      className={cn(
        "mx-auto grid size-full grow items-start gap-4 p-4 pt-2",
        "md:grid-cols-[1fr_290px] md:overflow-hidden"
      )}
    >
      <VideoWorkspace />
      <aside className={cn("flex h-full min-w-0 flex-col gap-4", "md:overflow-hidden")}>
        <div
          className={cn(
            "flex min-w-0 flex-col rounded-md border bg-card p-1",
            "md:overflow-hidden"
          )}
        >
          <ScrollArea className="h-full min-w-0 p-2">
            <div className="flex min-w-0 grow flex-col gap-2 p-1">
              <h2 className={cn("text-xl font-semibold")}>Settings</h2>
              <VideoSettings />
            </div>
          </ScrollArea>
        </div>
        <CompressionDetailsCard />
      </aside>
    </div>
  );
}
