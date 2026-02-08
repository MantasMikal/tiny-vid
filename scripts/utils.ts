import { quote } from "zx";

export function formatError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function formatCommand(command: string, args: string[]): string {
  return [command, ...args].map((value) => quote(value)).join(" ");
}

export function envChanges(env: NodeJS.ProcessEnv): string[] {
  const changes: string[] = [];

  for (const [key, value] of Object.entries(env)) {
    if (process.env[key] === value) continue;
    changes.push(`${key}=${value ?? "<unset>"}`);
  }

  return changes.sort();
}
