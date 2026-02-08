import { useEffect, useState } from "react";

import { cn } from "@/lib/utils";
import { desktopClient } from "@/platform/desktop/client";

const MAC_TITLEBAR_INSET_PX = 28;

export function WindowFrame({ children }: { children: React.ReactNode }) {
  const [isMacDesktop, setIsMacDesktop] = useState(false);

  useEffect(() => {
    let mounted = true;
    void desktopClient
      .platform()
      .then((platform) => {
        if (mounted) {
          setIsMacDesktop(platform === "macos");
        }
      })
      .catch(() => {
        // Non-desktop runtimes do not expose the Electron bridge.
        if (mounted) {
          setIsMacDesktop(false);
        }
      });
    return () => {
      mounted = false;
    };
  }, []);

  return (
    <div
      className={cn(
        `flex grow flex-col select-none md:max-h-screen`,
        "transition-colors duration-300 ease-in-out"
      )}
    >
      {isMacDesktop ? (
        <div
          aria-hidden
          className="window-drag-region shrink-0"
          style={{ height: `${String(MAC_TITLEBAR_INSET_PX)}px` }}
        />
      ) : null}
      <div className="window-no-drag-region flex min-h-0 grow flex-col">{children}</div>
    </div>
  );
}
