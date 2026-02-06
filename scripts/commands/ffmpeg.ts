import { type FfmpegProfile, profileBuildScript } from "../ffmpeg-profile.ts";
import { type CommandContext, runCommand } from "../runtime.ts";
import { runPrepare } from "../standalone.ts";

export interface FfmpegCommandOptions {
  profile: FfmpegProfile;
}

export async function runFfmpegPrepareCommand(
  context: CommandContext,
  options: FfmpegCommandOptions
): Promise<number> {
  await runPrepare(options.profile, context);
  return 0;
}

export async function runFfmpegBuildCommand(
  context: CommandContext,
  options: FfmpegCommandOptions
): Promise<number> {
  if (process.platform !== "darwin") {
    throw new Error("Source FFmpeg build is supported only on macOS in this project");
  }

  const script = profileBuildScript(options.profile);
  return runCommand(context, "bash", [script]);
}
