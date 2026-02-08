import { type FfmpegProfile } from "../ffmpeg-profile.ts";
import { type CommandContext, runCommand } from "../runtime.ts";
import {
  type BuildMode,
  type BuildPlatform,
  validateStandaloneOptions,
} from "../standalone.ts";

export interface BuildCommandOptions {
  mode: BuildMode;
  profile?: FfmpegProfile;
  platform: BuildPlatform;
}

const PLATFORM_ARGS: Record<Exclude<BuildPlatform, "auto">, string> = {
  macos: "--mac",
  windows: "--win",
  linux: "--linux",
};

function platformArgs(platform: BuildPlatform): string[] {
  if (platform === "auto") {
    return [];
  }
  return ["--", PLATFORM_ARGS[platform]];
}

export async function runBuildCommand(
  context: CommandContext,
  options: BuildCommandOptions
): Promise<number> {
  const profile = validateStandaloneOptions({
    mode: options.mode,
    profile: options.profile,
    platform: options.platform === "auto" ? undefined : options.platform,
  });

  const args = ["scripts/electron/build.ts", "--mode", options.mode];
  if (profile) {
    args.push("--profile", profile);
  }
  args.push(...platformArgs(options.platform));

  return runCommand(context, "node", args);
}
