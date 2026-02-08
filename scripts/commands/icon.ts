import { type CommandContext, runCommand } from "../runtime.ts";

export function runIconGenerateCommand(context: CommandContext): Promise<number> {
  return runCommand(context, "node", ["scripts/generate-icons.ts"]);
}
