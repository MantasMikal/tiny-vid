import { join } from "node:path";

import { type FfmpegProfile } from "../ffmpeg-profile.ts";
import {
  bundlePath,
  cleanBundle,
  type CommandContext,
  copyFileToOutput,
  ensureDir,
  envWithoutCi,
  firstFileWithExtension,
  readVersion,
  runCommand,
} from "../runtime.ts";
import {
  assertStandaloneProfile,
  type BuildMode,
  type BuildPlatform,
  buildTauriArgs,
  runPrepare,
  validateStandaloneOptions,
} from "../standalone.ts";
import { archTag } from "../utils.ts";

export interface BuildCommandOptions {
  mode: BuildMode;
  platform: BuildPlatform;
  profile?: FfmpegProfile;
}

const PLATFORM_ARTIFACTS = {
  macos: [{ dir: "dmg", ext: ".dmg" }] as const,
  windows: [
    { dir: "msi", ext: ".msi" },
    { dir: "nsis", ext: ".exe" },
  ] as const,
} as const;

function detectBuildPlatform(): Exclude<BuildPlatform, "auto"> {
  if (process.platform === "darwin") return "macos";
  if (process.platform === "win32") return "windows";
  if (process.platform === "linux") return "linux";
  throw new Error(`Unsupported platform: ${process.platform}`);
}

async function runTauriBuild(
  context: CommandContext,
  options: { mode: BuildMode; profile: FfmpegProfile | null }
): Promise<number> {
  if (options.mode === "system") {
    return runCommand(context, "yarn", ["tauri", "build"], {
      check: false,
      env: envWithoutCi(process.env),
    });
  }

  assertStandaloneProfile(options.profile);
  await runPrepare(options.profile, context);
  return runCommand(context, "yarn", buildTauriArgs(options.profile, "build"), {
    check: false,
    env: envWithoutCi(process.env),
  });
}

async function buildPlatform(
  context: CommandContext,
  options: { mode: BuildMode; platform: BuildPlatform; profile: FfmpegProfile | null },
  platform: "macos" | "windows"
): Promise<number> {
  if (platform === "macos" && process.platform !== "darwin") {
    throw new Error("macOS build must run on macOS");
  }
  if (platform === "windows" && process.platform !== "win32") {
    throw new Error("Windows build must run on Windows");
  }
  if (platform === "windows" && options.mode === "standalone" && options.profile !== "gpl") {
    throw new Error("Windows standalone supports only --profile gpl");
  }

  const version = readVersion(context);
  const arch = archTag();
  cleanBundle(context);

  const profile = options.mode === "standalone" ? options.profile : null;
  const suffix =
    profile !== null ? `${platform}-standalone-${profile}-${arch}` : `${platform}-${arch}`;

  const modeLabel = profile !== null ? `standalone ${profile}` : "system";
  console.log(`Building ${platform} (${modeLabel}) - version ${version} (${arch})`);

  const status = await runTauriBuild(context, options);

  const outputDir = join(context.rootDir, "releases", platform);
  ensureDir(context, outputDir);
  const bundleBase = bundlePath(context);

  for (const { dir, ext } of PLATFORM_ARTIFACTS[platform]) {
    const file = firstFileWithExtension(join(bundleBase, dir), ext);
    if (file) {
      const output = join(outputDir, `Tiny-Vid-${version}-${suffix}${ext}`);
      copyFileToOutput(context, file, output);
      console.log(`Output: ${output}`);
    }
  }

  return status;
}

async function buildLinux(
  context: CommandContext,
  options: { mode: BuildMode; platform: BuildPlatform; profile: FfmpegProfile | null }
): Promise<number> {
  if (process.platform !== "linux") {
    throw new Error("Linux build must run on Linux");
  }
  if (options.mode === "standalone") {
    throw new Error("Bundled standalone profiles are not supported on Linux in this project");
  }

  const version = readVersion(context);
  const arch = archTag();

  console.log(`Building Linux .deb - version ${version} (${arch})`);

  cleanBundle(context);
  const status = await runTauriBuild(context, { mode: "system", profile: null });

  const bundleDir = bundlePath(context, "deb");
  const deb = firstFileWithExtension(bundleDir, ".deb");
  if (!deb) {
    if (context.dryRun) {
      console.log(`[dry-run] expected a .deb in ${bundleDir}`);
      return 0;
    }

    throw new Error(`No .deb found in ${bundleDir}`);
  }

  const outputDir = join(context.rootDir, "releases", "linux");
  ensureDir(context, outputDir);
  const output = join(outputDir, `Tiny-Vid-${version}-linux-${arch}.deb`);
  copyFileToOutput(context, deb, output);
  console.log(`Output: ${output}`);

  return status;
}

export async function runBuildCommand(
  context: CommandContext,
  options: BuildCommandOptions
): Promise<number> {
  const platform = options.platform === "auto" ? detectBuildPlatform() : options.platform;
  const profile = validateStandaloneOptions({
    mode: options.mode,
    profile: options.profile,
    platform,
  });

  const resolved = { mode: options.mode, platform: options.platform, profile };

  if (platform === "macos") {
    return buildPlatform(context, resolved, "macos");
  }
  if (platform === "windows") {
    return buildPlatform(context, resolved, "windows");
  }
  return buildLinux(context, resolved);
}
