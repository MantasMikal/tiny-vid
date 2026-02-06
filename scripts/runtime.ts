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
  srcTauriDir: string;
}

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
export const ROOT_DIR = join(__dirname, "..");
export const SRC_TAURI_DIR = join(ROOT_DIR, "src-tauri");
const BUNDLE_DIR = "target/release/bundle";

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
    srcTauriDir: SRC_TAURI_DIR,
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

export function readVersion(context: CommandContext): string {
  const configPath = join(context.srcTauriDir, "tauri.conf.json");
  const parsed = fs.readJsonSync(configPath) as { version?: unknown };

  if (typeof parsed.version !== "string" || parsed.version.length === 0) {
    throw new Error(`Could not read version from ${configPath}`);
  }

  return parsed.version;
}

export function firstFileWithExtension(dir: string, ext: string): string | null {
  if (!fs.existsSync(dir)) return null;

  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (entry.isFile() && entry.name.endsWith(ext)) {
      return join(dir, entry.name);
    }
  }

  return null;
}

export function bundlePath(context: CommandContext, subdir?: string): string {
  const base = join(context.srcTauriDir, BUNDLE_DIR);
  return subdir ? join(base, subdir) : base;
}

export function withDryRun(context: CommandContext, message: string, fn: () => void): void {
  if (context.dryRun) {
    console.log(`[dry-run] ${message}`);
    return;
  }
  fn();
}

export function cleanBundle(context: CommandContext): void {
  const path = bundlePath(context);
  withDryRun(context, `rm -rf ${path}`, () => fs.removeSync(path));
}

export function ensureDir(context: CommandContext, dir: string): void {
  withDryRun(context, `mkdir -p ${dir}`, () => fs.ensureDirSync(dir));
}

export function copyFileToOutput(
  context: CommandContext,
  inputPath: string,
  outputPath: string
): void {
  withDryRun(context, `cp ${inputPath} ${outputPath}`, () => {
    fs.ensureDirSync(dirname(outputPath));
    fs.copySync(inputPath, outputPath);
  });
}

export function envWithoutCi(baseEnv: NodeJS.ProcessEnv): NodeJS.ProcessEnv {
  const env = { ...baseEnv };
  delete env.CI;
  return env;
}
