import { fs } from "zx";

import {
  assertProfileSupportedOnTarget,
  type FfmpegProfile,
  getTargetTriple,
  profileFfmpegPath,
  profilePrereqCommand,
  profileTauriConfig,
} from "./ffmpeg-profile.ts";
import { prepareFfmpeg } from "./prepare-ffmpeg.ts";
import { type CommandContext, runCommand } from "./runtime.ts";

export const BUILD_MODES = ["system", "standalone"] as const;
export const BUILD_PLATFORMS = ["auto", "macos", "windows", "linux"] as const;
export const TEST_SUITES = [
  "unit",
  "integration-smoke",
  "integration-contract",
  "all",
  "discovery",
] as const;

export type BuildMode = (typeof BUILD_MODES)[number];
export type BuildPlatform = (typeof BUILD_PLATFORMS)[number];
export type TestSuite = (typeof TEST_SUITES)[number];

const VIDEO_TOOLBOX_PROBE_ARGS = [
  "-loglevel",
  "error",
  "-y",
  "-f",
  "lavfi",
  "-i",
  "testsrc=duration=0.1:size=16x16:rate=1",
  "-frames:v",
  "1",
  "-c:v",
  "h264_videotoolbox",
  "-f",
  "null",
  "-",
] as const;

function resolveProfileForMode(
  mode: BuildMode,
  profile: FfmpegProfile | undefined
): FfmpegProfile | null {
  if (mode === "system") {
    if (profile) {
      throw new Error("--profile is only valid when --mode standalone");
    }

    return null;
  }

  if (!profile) {
    throw new Error("--profile is required when --mode standalone");
  }

  return profile;
}

function validateSuiteForMode(mode: BuildMode, suite: TestSuite): void {
  if (mode === "system" && suite === "integration-contract") {
    throw new Error("integration-contract suite is only available for --mode standalone");
  }
}

function validateStandaloneProfileForBuild(
  profile: FfmpegProfile,
  platform: Exclude<BuildPlatform, "auto">
): void {
  if (platform === "linux") {
    throw new Error("Bundled standalone profiles are not supported on Linux in this project");
  }

  if (platform === "windows" && profile === "lgpl-vt") {
    throw new Error("lgpl-vt profile is macOS-only");
  }
}

function validateStandaloneTarget(profile: FfmpegProfile | null): void {
  if (!profile) return;

  const target = getTargetTriple();
  assertProfileSupportedOnTarget(profile, target);
}

export function assertStandaloneProfile(
  profile: FfmpegProfile | null
): asserts profile is FfmpegProfile {
  if (!profile) {
    throw new Error("Missing standalone profile");
  }
}

export function validateStandaloneOptions(opts: {
  mode: BuildMode;
  profile: FfmpegProfile | undefined;
  suite?: TestSuite;
  platform?: Exclude<BuildPlatform, "auto">;
}): FfmpegProfile | null {
  const profile = resolveProfileForMode(opts.mode, opts.profile);
  if (opts.suite !== undefined) {
    validateSuiteForMode(opts.mode, opts.suite);
  }
  if (profile) {
    validateStandaloneTarget(profile);
    if (opts.platform !== undefined) validateStandaloneProfileForBuild(profile, opts.platform);
  }
  return profile;
}

export async function runPrepare(
  profile: FfmpegProfile,
  context: CommandContext
): Promise<void> {
  if (context.dryRun) {
    await runCommand(context, "node", ["scripts/prepare-ffmpeg.ts", "--ffmpeg-profile", profile]);
    return;
  }

  await prepareFfmpeg(profile);
}

async function probeVideoToolbox(context: CommandContext, ffmpegPath: string): Promise<boolean> {
  const status = await runCommand(context, ffmpegPath, [...VIDEO_TOOLBOX_PROBE_ARGS], {
    check: false,
    stdio: "pipe",
  });

  return status === 0;
}

async function resolveStandaloneFfmpegPath(
  context: CommandContext,
  profile: FfmpegProfile
): Promise<string> {
  const target = getTargetTriple();
  assertProfileSupportedOnTarget(profile, target);

  const ffmpegPath = profileFfmpegPath(context.rootDir, profile, target);
  if (!fs.existsSync(ffmpegPath)) {
    if (context.dryRun) {
      console.log(`[dry-run] missing bundled ffmpeg would fail here: ${ffmpegPath}`);
      return ffmpegPath;
    }

    const buildCommand = profilePrereqCommand(profile, target);
    throw new Error(
      `Bundled FFmpeg not found for profile ${profile} (${target}): ${ffmpegPath}\n` +
        `Build/prep binaries first: ${buildCommand}`
    );
  }

  if (profile === "lgpl-vt") {
    if (context.dryRun) {
      console.log("[dry-run] skipping VideoToolbox preflight probe");
      return ffmpegPath;
    }

    if (!(await probeVideoToolbox(context, ffmpegPath))) {
      throw new Error(
        `VideoToolbox probe failed for ${ffmpegPath}. LGPL commands require usable hardware VideoToolbox runtime (no software fallback).`
      );
    }
  }

  return ffmpegPath;
}

export async function setupStandaloneEnv(
  context: CommandContext,
  profile: FfmpegProfile
): Promise<NodeJS.ProcessEnv> {
  await runPrepare(profile, context);
  const ffmpegPath = await resolveStandaloneFfmpegPath(context, profile);
  return { ...process.env, FFMPEG_PATH: ffmpegPath };
}

export function buildTauriArgs(profile: FfmpegProfile, subcommand: "build" | "dev"): string[] {
  const { config, features } = profileTauriConfig(profile);
  const args = ["tauri", subcommand, "--config", config];
  if (features?.length) args.push("--features", features.join(","));
  return args;
}
