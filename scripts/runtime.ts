import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { $, fs, useBash, usePowerShell } from "zx";

import { envChanges, formatCommand } from "./utils.ts";

export type StdioMode = "inherit" | "pipe";

export interface GlobalOptions {
  dryRun: boolean;
  verbose: boolean;
}

export interface RunOptions {
  check?: boolean;
  cwd?: string;
  env?: NodeJS.ProcessEnv;
  stdio?: StdioMode;
}

export interface CommandContext extends GlobalOptions {
  rootDir: string;
}

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
export const ROOT_DIR = join(__dirname, "..");
const ELECTRON_RELEASE_DIR = join(ROOT_DIR, "releases", "electron");
const ELECTRON_RESOURCE_BIN_DIR = join(ROOT_DIR, "electron", "resources", "bin");

let shellConfigured = false;

export function configureShell(verbose: boolean): void {
  if (!shellConfigured) {
    if (process.platform === "win32") {
      usePowerShell();
    } else {
      useBash();
    }

    shellConfigured = true;
  }

  $.verbose = verbose;
}

export function createContext(options: GlobalOptions): CommandContext {
  return {
    ...options,
    rootDir: ROOT_DIR,
  };
}

export async function runCommand(
  context: CommandContext,
  command: string,
  args: string[],
  options: RunOptions = {}
): Promise<number> {
  const cwd = options.cwd ?? context.rootDir;
  const env = options.env ?? process.env;
  const check = options.check ?? true;
  const stdio = options.stdio ?? "inherit";

  if (context.dryRun) {
    const prefix = cwd === context.rootDir ? "[dry-run]" : `[dry-run][cwd=${cwd}]`;
    console.log(`${prefix} ${formatCommand(command, args)}`);

    const changes = envChanges(env);
    if (changes.length > 0) {
      console.log(`[dry-run][env] ${changes.join(" ")}`);
    }

    return 0;
  }

  const task = $({ cwd, env, stdio })`${command} ${args}`;

  if (check) {
    const output = await task;
    return output.exitCode ?? 1;
  }

  const output = await task.nothrow();
  return output.exitCode ?? 1;
}

export function withDryRun(context: CommandContext, message: string, fn: () => void): void {
  if (context.dryRun) {
    console.log(`[dry-run] ${message}`);
    return;
  }
  fn();
}

export function cleanBundle(context: CommandContext): void {
  withDryRun(context, `rm -rf ${ELECTRON_RELEASE_DIR}`, () => fs.removeSync(ELECTRON_RELEASE_DIR));
  withDryRun(context, `rm -rf ${ELECTRON_RESOURCE_BIN_DIR}`, () =>
    fs.removeSync(ELECTRON_RESOURCE_BIN_DIR)
  );
}
