/**
 * Resolves a build variant to the override config path. Tauri merges this file
 * with the default config (base + platform) when using `tauri build --config <path>`.
 *
 * Usage: node scripts/merge-config.ts [--path] <variant>
 * Variants: full | bare | macos-bare | macos-full | macos-lgpl | windows-bare | windows-full | linux-bare
 */

import { join } from "node:path";

const VALID_VARIANTS = [
  "macos-bare",
  "macos-full",
  "macos-lgpl",
  "windows-bare",
  "windows-full",
  "linux-bare",
] as const;

type Variant = (typeof VALID_VARIANTS)[number];

/** Resolve "full" | "bare" to platform-specific variant. */
function resolveVariant(variant: string): Variant {
  const platform = process.platform;
  if (variant === "full") {
    if (platform === "darwin") return "macos-full";
    if (platform === "win32") return "windows-full";
    throw new Error("yarn build:full is only supported on macOS and Windows");
  }
  if (variant === "bare") {
    if (platform === "darwin") return "macos-bare";
    if (platform === "win32") return "windows-bare";
    if (platform === "linux") return "linux-bare";
    return "macos-bare"; // fallback for other unix
  }
  if (VALID_VARIANTS.includes(variant as Variant)) return variant as Variant;
  throw new Error(`Unknown variant: ${variant}`);
}

function main(): void {
  const pathOnly = process.argv[2] === "--path";
  const arg = pathOnly ? process.argv[3] : process.argv[2];
  if (!arg) {
    console.error(
      `usage: merge-config.ts [--path] <variant>\nvariants: full | bare | ${VALID_VARIANTS.join(" | ")}`
    );
    process.exit(1);
  }
  const variant = resolveVariant(arg);
  const pathRelative = join("src-tauri", "overrides", `${variant}.json`);
  console.log(pathRelative);
}

main();
