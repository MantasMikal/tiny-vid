import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";

type Theme = "light" | "dark";

function getSystemTheme(): Theme {
  if (typeof window === "undefined") return "dark";
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

function applyTheme(theme: Theme) {
  const root = document.documentElement;
  if (theme === "dark") {
    root.classList.add("dark");
  } else {
    root.classList.remove("dark");
  }
}

async function applyNativeWindowTheme(theme: Theme) {
  try {
    const appWindow = getCurrentWindow();
    const color =
      theme === "dark"
        ? { red: 10, green: 10, blue: 10, alpha: 1 }
        : { red: 255, green: 255, blue: 255, alpha: 1 };
    await appWindow.setBackgroundColor(color);
  } catch {
    // Not in Tauri or setBackgroundColor unsupported
  }
}

/**
 * Syncs app theme with user system preference.
 * Uses Tauri's native theme API when available (more reliable on all platforms),
 * falls back to prefers-color-scheme media query for web/Vite dev.
 */
export function useTheme() {
  const [theme, setTheme] = useState<Theme>(() => getSystemTheme());
  const [isTauri, setIsTauri] = useState<boolean | null>(null);

  useEffect(() => {
    let cleanup: (() => void) | undefined;

    async function initTheme() {
      try {
        const appWindow = getCurrentWindow();
        const resolvedTheme = await appWindow.theme();

        if (resolvedTheme !== null) {
          setTheme(resolvedTheme);
          applyTheme(resolvedTheme);
          void applyNativeWindowTheme(resolvedTheme);
        } else {
          const systemTheme = getSystemTheme();
          setTheme(systemTheme);
          applyTheme(systemTheme);
          void applyNativeWindowTheme(systemTheme);
        }

        cleanup = await appWindow.onThemeChanged(({ payload }) => {
          setTheme(payload);
          applyTheme(payload);
          void applyNativeWindowTheme(payload);
        });

        setIsTauri(true);
      } catch {
        setIsTauri(false);
        const systemTheme = getSystemTheme();
        setTheme(systemTheme);
        applyTheme(systemTheme);

        const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
        const handleChange = () => {
          const newTheme = mediaQuery.matches ? "dark" : "light";
          setTheme(newTheme);
          applyTheme(newTheme);
        };

        mediaQuery.addEventListener("change", handleChange);
        cleanup = () => {
          mediaQuery.removeEventListener("change", handleChange);
        };
      }
    }

    void initTheme();

    return () => {
      cleanup?.();
    };
  }, []);

  return { theme, isTauri };
}
