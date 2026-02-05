/**
 * Removes the Tauri bundle output directory to avoid stale sidecar binaries
 * when switching between build variants (default vs standalone vs lgpl).
 */
import { rmSync } from "node:fs";
import { join } from "node:path";

const bundlePath = join(
  process.cwd(),
  "src-tauri",
  "target",
  "release",
  "bundle"
);
rmSync(bundlePath, { recursive: true, force: true });
