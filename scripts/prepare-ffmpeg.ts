/**
 * Prepares FFmpeg binaries for bundling. Run before `tauri build` for macOS/Windows.
 *
 * - Linux / bare: No-op (platform overrides set externalBin to [])
 * - Windows: Downloads BtbN win64-gpl, extracts to src-tauri/binaries/
 * - macOS full: Expects output from build-ffmpeg-full-macos.sh; fails if missing
 * - macOS lgpl-macos: Expects output from build-ffmpeg-lgpl-macos.sh; fails if missing
 *
 * Caches BtbN downloads in ~/.cache/tiny-vid/ffmpeg (or TINY_VID_FFMPEG_CACHE). Verifies checksums when BtbN provides checksums.sha256.
 *
 * Env: TARGET, CARGO_BUILD_TARGET (target triple); TINY_VID_LGPL_MACOS (truthy = lgpl-macos)
 *       TINY_VID_FFMPEG_CACHE (optional) cache directory for downloads
 * Flags: --stub Create placeholder binaries (for build pass without download)
 */

import { execSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmodSync,
  copyFileSync,
  createReadStream,
  createWriteStream,
  existsSync,
  mkdirSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { homedir, platform } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const ROOT = join(__dirname, "..");
const BINARIES_DIR = join(ROOT, "src-tauri", "binaries");

/** BtbN release tag. Use "latest" for newest; pin to a specific tag for reproducibility when available. */
const BtbN_RELEASE = process.env.TINY_VID_BTBN_RELEASE ?? "latest";
const BtbN_BASE = `https://github.com/BtbN/FFmpeg-Builds/releases/download/${BtbN_RELEASE}`;

function getCacheDir(): string {
  if (process.env.TINY_VID_FFMPEG_CACHE) {
    return process.env.TINY_VID_FFMPEG_CACHE;
  }
  if (platform() === "win32") {
    const local =
      process.env.LOCALAPPDATA ?? join(homedir(), "AppData", "Local");
    return join(local, "tiny-vid", "cache", "ffmpeg");
  }
  return join(homedir(), ".cache", "tiny-vid", "ffmpeg");
}

interface BtbNAsset { url: string; filename: string; ext: "tar.xz" | "zip" }

function getTargetTriple(): string {
  return (
    process.env.CARGO_BUILD_TARGET ??
    process.env.TARGET ??
    execSync("rustc --print host-tuple", { encoding: "utf8" }).trim()
  );
}

function isLgplMacosBuild(): boolean {
  return !!process.env.TINY_VID_LGPL_MACOS;
}

function isLinux(target: string): boolean {
  return target.includes("linux");
}

function isWindows(target: string): boolean {
  return target.includes("windows");
}

function isMacOs(target: string): boolean {
  return target.includes("darwin");
}

function getBtbNAsset(target: string): BtbNAsset | null {
  if (target.includes("x86_64") && target.includes("windows")) {
    const filename = "ffmpeg-master-latest-win64-gpl.zip";
    return {
      url: `${BtbN_BASE}/${filename}`,
      filename,
      ext: "zip",
    };
  }
  return null;
}

function getSidecarSuffix(target: string): string {
  const exe = isWindows(target) ? ".exe" : "";
  return `${target}${exe}`;
}

/** Fetch BtbN checksums.sha256; returns Map<filename, expectedSha256> or null if unavailable. */
async function fetchChecksums(): Promise<Map<string, string> | null> {
  try {
    const res = await fetch(`${BtbN_BASE}/checksums.sha256`, {
      redirect: "follow",
      signal: AbortSignal.timeout(15_000),
    });
    if (!res.ok) return null;
    const text = await res.text();
    const map = new Map<string, string>();
    for (const line of text.split("\n")) {
      const trimmed = line.trim();
      if (!trimmed) continue;
      const space = trimmed.indexOf("  ");
      const star = trimmed.indexOf(" *");
      const sep = space >= 0 && (star < 0 || space < star) ? space : star;
      if (sep < 0) continue;
      const hash = trimmed.slice(0, sep).trim().replace(/\*$/, "").trim();
      const name = trimmed
        .slice(sep + 1)
        .trim()
        .replace(/^\*/, "")
        .trim();
      if (hash && name && /^[a-fA-F0-9]{64}$/.test(hash)) {
        map.set(name, hash.toLowerCase());
      }
    }
    return map.size > 0 ? map : null;
  } catch {
    return null;
  }
}

function sha256File(path: string): Promise<string> {
  return new Promise((resolve, reject) => {
    const hash = createHash("sha256");
    const rs = createReadStream(path);
    rs.on("data", (chunk) => hash.update(chunk));
    rs.on("end", () => { resolve(hash.digest("hex")); });
    rs.on("error", reject);
  });
}

async function download(url: string, dest: string): Promise<void> {
  const res = await fetch(url, { redirect: "follow" });
  if (!res.ok) throw new Error(`Download failed: ${String(res.status)} ${url}`);
  const buf = Buffer.from(await res.arrayBuffer());
  mkdirSync(dirname(dest), { recursive: true });
  const ws = createWriteStream(dest);
  ws.write(buf);
  ws.end();
  await new Promise<void>((resolve, reject) => {
    ws.on("finish", () => { resolve(); });
    ws.on("error", reject);
  });
}

function extractTarXz(archivePath: string, outDir: string): void {
  const r = spawnSync("tar", ["-xJf", archivePath, "-C", outDir], {
    stdio: "inherit",
  });
  if (r.status !== 0) throw new Error(`tar exited ${String(r.status ?? "unknown")}`);
}

function extractZip(archivePath: string, outDir: string): void {
  if (process.platform === "win32") {
    const r = spawnSync(
      "powershell",
      [
        "-Command",
        `Expand-Archive -Path '${archivePath.replace(/'/g, "''")}' -DestinationPath '${outDir.replace(/'/g, "''")}'`,
      ],
      { stdio: "inherit" }
    );
    if (r.status !== 0)
      throw new Error(`Expand-Archive exited ${String(r.status ?? "unknown")}`);
  } else {
    const r = spawnSync("unzip", ["-o", archivePath, "-d", outDir], {
      stdio: "inherit",
    });
    if (r.status !== 0)
      throw new Error(`unzip exited ${String(r.status ?? "unknown")}`);
  }
}

function findFfmpegInExtracted(extractDir: string): string | null {
  return findBinaryInExtracted(extractDir, "ffmpeg");
}

function findBinaryInExtracted(extractDir: string, baseName: string): string | null {
  const exe = process.platform === "win32" ? ".exe" : "";
  const find = (dir: string): string | null => {
    const entries = readdirSync(dir, { withFileTypes: true });
    for (const e of entries) {
      const p = join(dir, e.name);
      if (e.isDirectory()) {
        const found = find(p);
        if (found) return found;
      } else if (e.name === baseName || e.name === `${baseName}${exe}`) {
        return p;
      }
    }
    return null;
  };
  return find(extractDir);
}

async function prepareBtbN(target: string): Promise<void> {
  const asset = getBtbNAsset(target);
  if (!asset) throw new Error(`No BtbN asset for target: ${target}`);

  const suffix = getSidecarSuffix(target);
  const ffmpegDest = join(BINARIES_DIR, `ffmpeg-${suffix}`);
  const ffprobeDest = join(BINARIES_DIR, `ffprobe-${suffix}`);

  if (existsSync(ffmpegDest) && existsSync(ffprobeDest)) {
    console.log(`FFmpeg binaries already exist for ${target}, skipping`);
    return;
  }

  const cacheDir = getCacheDir();
  const archivePath = join(cacheDir, asset.filename);
  mkdirSync(cacheDir, { recursive: true });
  mkdirSync(BINARIES_DIR, { recursive: true });

  const checksums = await fetchChecksums();
  const verify = async (path: string): Promise<void> => {
    const expected = checksums?.get(asset.filename);
    if (!expected) return;
    const actual = await sha256File(path);
    if (actual !== expected) {
      throw new Error(
        `Checksum mismatch for ${asset.filename}: expected ${expected}, got ${actual}. Delete cache and retry: ${path}`
      );
    }
  };

  if (!existsSync(archivePath)) {
    console.log(`Downloading ${asset.url} to cache...`);
    await download(asset.url, archivePath);
  } else {
    console.log(`Using cached ${asset.filename}`);
  }
  await verify(archivePath);

  const extractDir = join(BINARIES_DIR, "extract");
  mkdirSync(extractDir, { recursive: true });
  try {
    if (asset.ext === "zip") {
      extractZip(archivePath, extractDir);
    } else {
      extractTarXz(archivePath, extractDir);
    }

    const ffmpegPath = findFfmpegInExtracted(extractDir);
    if (!ffmpegPath) throw new Error("ffmpeg not found in archive");

    const extractParent = dirname(ffmpegPath);

    copyFileSync(ffmpegPath, ffmpegDest);
    const ffprobePath = join(
      extractParent,
      isWindows(target) ? "ffprobe.exe" : "ffprobe"
    );
    if (!existsSync(ffprobePath))
      throw new Error("ffprobe not found in archive");
    copyFileSync(ffprobePath, ffprobeDest);
  } finally {
    rmSync(extractDir, { recursive: true, force: true });
  }

  console.log(`Prepared ffmpeg and ffprobe for ${target}`);
}

/** macOS full: expects binaries from build-ffmpeg-full-macos.sh. */
function prepareFullMacOs(target: string): void {
  const suffix = getSidecarSuffix(target);
  const ffmpegDest = join(BINARIES_DIR, `ffmpeg-${suffix}`);
  const ffprobeDest = join(BINARIES_DIR, `ffprobe-${suffix}`);

  if (!existsSync(ffmpegDest) || !existsSync(ffprobeDest)) {
    throw new Error(
      `macOS full build requires FFmpeg built from source. Run yarn build-ffmpeg-full-macos first. ` +
        `Expected: ${ffmpegDest}, ${ffprobeDest}`
    );
  }
  console.log(`Using existing full FFmpeg binaries for ${target}`);
}

function prepareLgplMacOs(target: string): void {
  const suffix = getSidecarSuffix(target);
  const ffmpegDest = join(BINARIES_DIR, `ffmpeg-${suffix}`);
  const ffprobeDest = join(BINARIES_DIR, `ffprobe-${suffix}`);

  if (!existsSync(ffmpegDest) || !existsSync(ffprobeDest)) {
    throw new Error(
      `lgpl-macos build requires custom FFmpeg. Run yarn build-ffmpeg-lgpl-macos first. ` +
        `Expected: ${ffmpegDest}, ${ffprobeDest}`
    );
  }
  console.log(`Using existing lgpl-macos FFmpeg binaries for ${target}`);
}

function createStubBinaries(target: string): void {
  const suffix = getSidecarSuffix(target);
  const ffmpegDest = join(BINARIES_DIR, `ffmpeg-${suffix}`);
  const ffprobeDest = join(BINARIES_DIR, `ffprobe-${suffix}`);
  mkdirSync(BINARIES_DIR, { recursive: true });
  if (process.platform === "win32") {
    writeFileSync(ffmpegDest, "");
    writeFileSync(ffprobeDest, "");
  } else {
    const stub = "#!/bin/sh\nexit 0\n";
    writeFileSync(ffmpegDest, stub);
    writeFileSync(ffprobeDest, stub);
    chmodSync(ffmpegDest, 0o755);
    chmodSync(ffprobeDest, 0o755);
  }
  console.log(
    `Created stub binaries for ${target} (build only; run without --stub for real FFmpeg)`
  );
}

async function main(): Promise<void> {
  const stubOnly = process.argv.includes("--stub");
  const target = getTargetTriple();
  console.log(
    `Target: ${target}, lgpl-macos: ${String(isLgplMacosBuild())}, stub: ${String(stubOnly)}`
  );

  if (isLinux(target)) {
    console.log("Bare build: no FFmpeg bundling (platform overrides set externalBin to [])");
    return;
  }

  if (stubOnly) {
    createStubBinaries(target);
    return;
  }

  if (isWindows(target)) {
    await prepareBtbN(target);
    return;
  }

  if (isMacOs(target)) {
    if (isLgplMacosBuild()) {
      prepareLgplMacOs(target);
    } else {
      prepareFullMacOs(target);
    }
    return;
  }

  throw new Error(`Unsupported target: ${target}`);
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
