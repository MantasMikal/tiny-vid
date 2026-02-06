import { quote, which } from "zx";

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

export async function commandExists(name: string): Promise<boolean> {
  try {
    await which(name);
    return true;
  } catch {
    return false;
  }
}

export function archTag(): string {
  if (process.arch === "arm64") return "aarch64";
  if (process.arch === "x64") return "x86_64";
  return process.arch;
}
