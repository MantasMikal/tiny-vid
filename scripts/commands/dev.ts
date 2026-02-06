import { type FfmpegProfile } from "../ffmpeg-profile.ts";
import { type CommandContext,runCommand } from "../runtime.ts";
import {
  assertStandaloneProfile,
  type BuildMode,
  buildTauriArgs,
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

  if (options.mode === "system") {
    const env = { ...process.env, TINY_VID_USE_SYSTEM_FFMPEG: "1" };
    await runCommand(context, "yarn", ["tauri", "dev"], { env });
    return 0;
  }

  assertStandaloneProfile(profile);
  const env = await setupStandaloneEnv(context, profile);
  await runCommand(context, "yarn", buildTauriArgs(profile, "dev"), { env });
  return 0;
}
