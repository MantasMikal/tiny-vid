import { cleanBundle, type CommandContext } from "../runtime.ts";

export function runCleanBundleCommand(context: CommandContext): Promise<number> {
  cleanBundle(context);
  console.log("Removed Electron packaged outputs and resource sidecar binaries");
  return Promise.resolve(0);
}
