import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { runInheritedCommand } from "../process.ts";

type BuildMode = "system" | "standalone";
type StandaloneProfile = "gpl" | "lgpl-vt";

interface ParsedArgs {
  mode: BuildMode;
  profile: StandaloneProfile | null;
  builderArgs: string[];
}

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const ROOT_DIR = path.resolve(__dirname, "..", "..");
const NATIVE_DIR = path.join(ROOT_DIR, "native");
const CARGO_MANIFEST_PATH = path.join(NATIVE_DIR, "Cargo.toml");
const ELECTRON_RESOURCE_BIN_DIR = path.join(ROOT_DIR, "electron", "resources", "bin");
const YARN_CMD = process.platform === "win32" ? "yarn.cmd" : "yarn";

function printHelp(): void {
  console.log(`Usage: node scripts/electron/build.ts [options] [electron-builder args]

Options:
  --mode <system|standalone>    FFmpeg runtime mode (default: system)
  --profile <gpl|lgpl-vt|lgpl>  Required when --mode standalone
  --help                        Show this help

Examples:
  yarn build:electron
  yarn build:electron:standalone -- --mac
  node scripts/electron/build.ts --mode standalone --profile gpl -- --win --x64
`);
}

function parseMode(value: string): BuildMode {
  if (value === "system" || value === "standalone") {
    return value;
  }
  throw new Error(`Invalid --mode value: ${value}`);
}

function parseProfile(value: string): StandaloneProfile {
  if (value === "lgpl") {
    return "lgpl-vt";
  }
  if (value === "gpl" || value === "lgpl-vt") {
    return value;
  }
  throw new Error(`Invalid --profile value: ${value}`);
}

function parseArgs(argv: string[]): ParsedArgs {
  let mode: BuildMode = "system";
  let profile: StandaloneProfile | null = null;
  const builderArgs: string[] = [];

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--help" || arg === "-h") {
      printHelp();
      process.exit(0);
    }
    if (arg === "--mode") {
      const value = argv[i + 1];
      if (!value) {
        throw new Error("Missing value for --mode");
      }
      mode = parseMode(value);
      i += 1;
      continue;
    }
    if (arg === "--profile") {
      const value = argv[i + 1];
      if (!value) {
        throw new Error("Missing value for --profile");
      }
      profile = parseProfile(value);
      i += 1;
      continue;
    }
    if (arg === "--") {
      builderArgs.push(...argv.slice(i + 1));
      break;
    }
    builderArgs.push(arg);
  }

  if (mode === "standalone" && profile === null) {
    throw new Error("Standalone mode requires --profile gpl or --profile lgpl-vt (alias: lgpl)");
  }
  if (mode === "system" && profile !== null) {
    throw new Error("--profile is only valid with --mode standalone");
  }

  return { mode, profile, builderArgs };
}

function getHostTargetTriple(): string {
  const result = spawnSync("rustc", ["--print", "host-tuple"], {
    cwd: ROOT_DIR,
    encoding: "utf8",
  });
  if (result.status !== 0) {
    const stderr =
      typeof result.stderr === "string" && result.stderr.length > 0
        ? result.stderr
        : "Failed to resolve Rust host target triple";
    throw new Error(stderr);
  }
  const stdout = typeof result.stdout === "string" ? result.stdout : String(result.stdout);
  const target = stdout.trim();
  if (!target) {
    throw new Error("Rust host target triple is empty");
  }
  return target;
}

function isWindowsTarget(target: string): boolean {
  return target.includes("windows");
}

function isMacOsTarget(target: string): boolean {
  return target.includes("darwin");
}

function validateStandalonePolicy(target: string, profile: StandaloneProfile): void {
  if (target.includes("linux")) {
    throw new Error(
      `Standalone FFmpeg is not supported on Linux in this project (target ${target}). Use --mode system.`,
    );
  }
  if (!isWindowsTarget(target) && !isMacOsTarget(target)) {
    throw new Error(
      `Standalone FFmpeg is only supported on macOS and Windows in this project (target ${target}).`,
    );
  }
  if (profile === "lgpl-vt" && !isMacOsTarget(target)) {
    throw new Error(`FFmpeg profile lgpl-vt is macOS-only. Target ${target} is not supported.`);
  }
}

function sidecarBinaryName(): string {
  return process.platform === "win32" ? "tiny-vid-sidecar.exe" : "tiny-vid-sidecar";
}

function copyExecutable(sourcePath: string, targetPath: string): void {
  fs.copyFileSync(sourcePath, targetPath);
  if (process.platform !== "win32") {
    fs.chmodSync(targetPath, 0o755);
  }
}

function profileDirName(profile: StandaloneProfile): string {
  return profile === "lgpl-vt" ? "standalone-lgpl-vt" : "standalone-gpl";
}

function standaloneSuffixForTarget(target: string): string {
  if (target.includes("windows")) {
    return `${target}.exe`;
  }
  return target;
}

function ensureFile(pathToCheck: string, message: string): void {
  if (!fs.existsSync(pathToCheck)) {
    throw new Error(message);
  }
}

function copyMacOsLgplDylibs(binariesDir: string): void {
  const dylibs = fs
    .readdirSync(binariesDir)
    .filter((entry) => entry.toLowerCase().endsWith(".dylib"))
    .sort();

  if (dylibs.length === 0) {
    throw new Error(`Missing LGPL dynamic libraries in ${binariesDir}`);
  }

  for (const dylib of dylibs) {
    const source = path.join(binariesDir, dylib);
    ensureFile(source, `Missing LGPL dynamic library: ${source}`);
    fs.copyFileSync(source, path.join(ELECTRON_RESOURCE_BIN_DIR, dylib));
  }
}

function copyStandaloneFfmpeg(profile: StandaloneProfile, target: string): void {
  const binariesDir = path.join(NATIVE_DIR, "binaries", profileDirName(profile));
  const suffix = standaloneSuffixForTarget(target);
  const ffmpegSource = path.join(binariesDir, `ffmpeg-${suffix}`);
  const ffprobeSource = path.join(binariesDir, `ffprobe-${suffix}`);

  ensureFile(
    ffmpegSource,
    `Missing bundled ffmpeg at ${ffmpegSource}. Run: yarn tv ffmpeg prepare --profile ${profile}`,
  );
  ensureFile(
    ffprobeSource,
    `Missing bundled ffprobe at ${ffprobeSource}. Run: yarn tv ffmpeg prepare --profile ${profile}`,
  );

  copyExecutable(ffmpegSource, path.join(ELECTRON_RESOURCE_BIN_DIR, path.basename(ffmpegSource)));
  copyExecutable(ffprobeSource, path.join(ELECTRON_RESOURCE_BIN_DIR, path.basename(ffprobeSource)));

  const plainFfmpegName = process.platform === "win32" ? "ffmpeg.exe" : "ffmpeg";
  const plainFfprobeName = process.platform === "win32" ? "ffprobe.exe" : "ffprobe";
  copyExecutable(ffmpegSource, path.join(ELECTRON_RESOURCE_BIN_DIR, plainFfmpegName));
  copyExecutable(ffprobeSource, path.join(ELECTRON_RESOURCE_BIN_DIR, plainFfprobeName));

  if (profile === "lgpl-vt" && target.includes("darwin")) {
    copyMacOsLgplDylibs(binariesDir);
  }
}

async function main(): Promise<void> {
  const { mode, profile, builderArgs } = parseArgs(process.argv.slice(2));
  const target = getHostTargetTriple();

  if (mode === "standalone") {
    if (!profile) {
      throw new Error("Standalone mode requires --profile");
    }
    validateStandalonePolicy(target, profile);
  }

  await runInheritedCommand(YARN_CMD, ["icon"], { cwd: ROOT_DIR, env: process.env });
  await runInheritedCommand(YARN_CMD, ["build:vite"], { cwd: ROOT_DIR, env: process.env });

  const cargoArgs = [
    "build",
    "--manifest-path",
    CARGO_MANIFEST_PATH,
    "--release",
    "--bin",
    "tiny-vid-sidecar",
  ];
  if (profile === "lgpl-vt") {
    cargoArgs.push("--features", "lgpl");
  }
  await runInheritedCommand("cargo", cargoArgs, { cwd: ROOT_DIR, env: process.env });

  fs.rmSync(ELECTRON_RESOURCE_BIN_DIR, { recursive: true, force: true });
  fs.mkdirSync(ELECTRON_RESOURCE_BIN_DIR, { recursive: true });

  const sidecarSource = path.join(NATIVE_DIR, "target", "release", sidecarBinaryName());
  ensureFile(sidecarSource, `Missing built sidecar binary: ${sidecarSource}`);
  copyExecutable(sidecarSource, path.join(ELECTRON_RESOURCE_BIN_DIR, sidecarBinaryName()));

  if (mode === "standalone") {
    if (!profile) {
      throw new Error("Missing standalone profile");
    }
    copyStandaloneFfmpeg(profile, target);
  }

  await runInheritedCommand(YARN_CMD, ["electron-builder", "--publish", "never", ...builderArgs], {
    cwd: ROOT_DIR,
    env: process.env,
  });
}

try {
  await main();
} catch (error) {
  console.error(error);
  process.exit(1);
}
