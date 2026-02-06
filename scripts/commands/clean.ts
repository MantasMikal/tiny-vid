import { cleanBundle, type CommandContext } from "../runtime.ts";

export function runCleanBundleCommand(context: CommandContext): Promise<number> {
  cleanBundle(context);
  console.log("Removed src-tauri/target/release/bundle");
  return Promise.resolve(0);
}
