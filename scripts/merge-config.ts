/**
 * Resolves a build variant to the override config path. Tauri merges this file
 * with the default config (base + platform) when using `tauri build --config <path>`.
 *
 * Usage: node scripts/merge-config.ts [--path] <variant>
 * Variants: full | macos-full | macos-lgpl | windows-full (bare is the default; no override)
 */

import { join } from "node:path";

const VALID_VARIANTS = [
  "macos-full",
  "macos-lgpl",
  "windows-full",
] as const;

type Variant = (typeof VALID_VARIANTS)[number];

/** Resolve "full" to platform-specific variant. Bare is the default; no override. */
function resolveVariant(variant: string): Variant {
  if (variant === "bare") {
    console.error("merge-config: bare is the default; no override. Use: yarn tauri dev");
    process.exit(1);
  }
  const platform = process.platform;
  if (variant === "full") {
    if (platform === "darwin") return "macos-full";
    if (platform === "win32") return "windows-full";
    throw new Error("yarn build:full is only supported on macOS and Windows");
  }
  if (VALID_VARIANTS.includes(variant as Variant)) return variant as Variant;
  throw new Error(`Unknown variant: ${variant}`);
}

function main(): void {
  const pathOnly = process.argv[2] === "--path";
  const arg = pathOnly ? process.argv[3] : process.argv[2];
  if (!arg) {
    console.error(
      `usage: merge-config.ts [--path] <variant>\nvariants: full | ${VALID_VARIANTS.join(" | ")}\n(bare is the default; no --config needed)`
    );
    process.exit(1);
  }
  const variant = resolveVariant(arg);
  const pathRelative = join("src-tauri", "overrides", `${variant}.json`);
  console.log(pathRelative);
}

main();
