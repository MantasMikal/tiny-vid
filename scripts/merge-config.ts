/**
 * Resolves a build variant to the override config path. Tauri merges this file
 * with the default config (base + platform) when using `tauri build --config <path>`.
 *
 * Usage: node scripts/merge-config.ts [--path] <variant>
 * Variants: standalone | macos-standalone | lgpl | windows-standalone (default: no override)
 */

import { join } from "node:path";

const VALID_VARIANTS = [
  "macos-standalone",
  "lgpl",
  "windows-standalone",
] as const;

type Variant = (typeof VALID_VARIANTS)[number];

/** Resolve "standalone" to platform-specific variant. Default needs no override. */
function resolveVariant(variant: string): Variant {
  const platform = process.platform;
  if (variant === "standalone") {
    if (platform === "darwin") return "macos-standalone";
    if (platform === "win32") return "windows-standalone";
    throw new Error("yarn build:standalone is only supported on macOS and Windows");
  }
  if (VALID_VARIANTS.includes(variant as Variant)) return variant as Variant;
  throw new Error(`Unknown variant: ${variant}`);
}

function main(): void {
  const pathOnly = process.argv[2] === "--path";
  const arg = pathOnly ? process.argv[3] : process.argv[2];
  if (!arg) {
    console.error(
      `usage: merge-config.ts [--path] <variant>\nvariants: standalone | ${VALID_VARIANTS.join(" | ")}\n(default: no --config needed)`
    );
    process.exit(1);
  }
  const variant = resolveVariant(arg);
  const pathRelative = join("src-tauri", "overrides", `${variant}.json`);
  console.log(pathRelative);
}

main();
