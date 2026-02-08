import { type FfmpegProfile } from "../ffmpeg-profile.ts";
import { type CommandContext, runCommand } from "../runtime.ts";
import {
  type BuildMode,
  setupStandaloneEnv,
  validateStandaloneOptions,
} from "../standalone.ts";

export interface DevCommandOptions {
  mode: BuildMode;
  profile?: FfmpegProfile;
}

export async function runDevCommand(
  context: CommandContext,
  options: DevCommandOptions
): Promise<number> {
  const profile = validateStandaloneOptions({
    mode: options.mode,
    profile: options.profile,
  });

  if (!profile) {
    return runCommand(context, "node", ["scripts/electron/dev.ts"]);
  }

  const env = await setupStandaloneEnv(context, profile);
  return runCommand(context, "node", ["scripts/electron/dev.ts"], { env });
}
