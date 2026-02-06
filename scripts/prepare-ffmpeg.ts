/**
 * Prepares FFmpeg binaries for a standalone profile. Obtains binaries if missing
 * (Windows: download BtbN; macOS: build from source).
 *
 * Policy:
 * - standalone + gpl:
 *   - macOS: builds via scripts/build-ffmpeg-standalone-macos.sh if missing
 *   - Windows: downloads BtbN prebuilt archive if missing
 * - standalone + lgpl-vt:
 *   - macOS only: builds via scripts/build-ffmpeg-lgpl.sh if missing
 *
 * Canonical entry: yarn tv ffmpeg prepare --profile gpl|lgpl-vt
 * Direct invocation: node scripts/prepare-ffmpeg.ts --ffmpeg-profile gpl
 */

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFileSync,
  createReadStream,
  createWriteStream,
  existsSync,
  mkdirSync,
  readdirSync,
  rmSync,
  statSync,
} from "node:fs";
import { homedir, platform } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  assertProfileSupportedOnTarget,
  FFMPEG_PROFILES,
  type FfmpegProfile,
  getTargetTriple,
  isMacOsTarget,
  isWindowsTarget,
  profileBuildScript,
  profileFfmpegPath,
  profileFfprobePath,
  profileLgplDylibPaths,
} from "./ffmpeg-profile.ts";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const ROOT = join(__dirname, "..");

function parseProfileFromArgv(argv: string[]): FfmpegProfile {
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--ffmpeg-profile" && argv[i + 1]) {
      const profile = argv[i + 1];
      if (FFMPEG_PROFILES.includes(profile as FfmpegProfile)) {
        return profile as FfmpegProfile;
      }
      throw new Error(`Invalid --ffmpeg-profile: ${profile}. Choose: ${FFMPEG_PROFILES.join(", ")}`);
    }
  }
  throw new Error("Missing --ffmpeg-profile. Usage: node scripts/prepare-ffmpeg.ts --ffmpeg-profile gpl|lgpl-vt");
}

/** BtbN release tag (latest or pinned). */
const BtbN_RELEASE = process.env.TINY_VID_BTBN_RELEASE ?? "latest";
const BtbN_BASE = `https://github.com/BtbN/FFmpeg-Builds/releases/download/${BtbN_RELEASE}`;

interface BtbNAsset {
  url: string;
  filename: string;
}

function getCacheDir(): string {
  if (process.env.TINY_VID_FFMPEG_CACHE) {
    return process.env.TINY_VID_FFMPEG_CACHE;
  }
  if (platform() === "win32") {
    const local = process.env.LOCALAPPDATA ?? join(homedir(), "AppData", "Local");
    return join(local, "tiny-vid", "cache", "ffmpeg");
  }
  return join(homedir(), ".cache", "tiny-vid", "ffmpeg");
}

function getBtbNAsset(target: string, profile: FfmpegProfile): BtbNAsset | null {
  if (profile !== "gpl" || !isWindowsTarget(target)) {
    return null;
  }
  const filename = target.includes("aarch64")
    ? "ffmpeg-master-latest-winarm64-gpl.zip"
    : "ffmpeg-master-latest-win64-gpl.zip";
  return {
    url: `${BtbN_BASE}/${filename}`,
    filename,
  };
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
    rs.on("end", () => {
      resolve(hash.digest("hex"));
    });
    rs.on("error", reject);
  });
}

async function download(url: string, dest: string): Promise<void> {
  const res = await fetch(url, {
    redirect: "follow",
    signal: AbortSignal.timeout(5 * 60_000),
  });
  if (!res.ok) throw new Error(`Download failed: ${String(res.status)} ${url}`);
  const body = res.body;
  if (!body) throw new Error(`No response body: ${url}`);
  mkdirSync(dirname(dest), { recursive: true });
  const ws = createWriteStream(dest);
  const reader = body.getReader();
  try {
    // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition -- intentional loop with break
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      ws.write(Buffer.from(value));
    }
  } finally {
    reader.releaseLock();
  }
  ws.end();
  await new Promise<void>((resolve, reject) => {
    ws.on("finish", resolve);
    ws.on("error", reject);
  });
}

function extractZip(archivePath: string, outDir: string): void {
  if (process.platform === "win32") {
    const r = spawnSync(
      "powershell",
      [
        "-Command",
        `Expand-Archive -Path '${archivePath.replace(/'/g, "''")}' -DestinationPath '${outDir.replace(/'/g, "''")}'`,
      ],
      { stdio: "inherit" },
    );
    if (r.status !== 0) {
      throw new Error(`Expand-Archive exited ${String(r.status ?? "unknown")}`);
    }
  } else {
    const r = spawnSync("unzip", ["-o", archivePath, "-d", outDir], {
      stdio: "inherit",
    });
    if (r.status !== 0) throw new Error(`unzip exited ${String(r.status ?? "unknown")}`);
  }
}

function findBinaryInExtracted(extractDir: string, baseName: string): string | null {
  const exe = process.platform === "win32" ? ".exe" : "";
  const find = (dir: string): string | null => {
    const entries = readdirSync(dir, { withFileTypes: true });
    for (const entry of entries) {
      const current = join(dir, entry.name);
      if (entry.isDirectory()) {
        const nested = find(current);
        if (nested) return nested;
      } else if (entry.name === baseName || entry.name === `${baseName}${exe}`) {
        return current;
      }
    }
    return null;
  };
  return find(extractDir);
}

async function prepareBtbN(target: string, profile: FfmpegProfile): Promise<void> {
  const asset = getBtbNAsset(target, profile);
  if (!asset) throw new Error(`No BtbN asset for target/profile: ${target}/${profile}`);

  const ffmpegDest = profileFfmpegPath(ROOT, profile, target);
  const ffprobeDest = profileFfprobePath(ROOT, profile, target);
  const destDir = dirname(ffmpegDest);

  if (existsSync(ffmpegDest) && existsSync(ffprobeDest)) {
    console.log(`FFmpeg binaries already exist for ${profile} (${target}), skipping`);
    return;
  }

  const cacheDir = getCacheDir();
  const archivePath = join(cacheDir, asset.filename);
  mkdirSync(cacheDir, { recursive: true });
  mkdirSync(destDir, { recursive: true });

  const checksums = await fetchChecksums();
  const verify = async (path: string): Promise<void> => {
    const expected = checksums?.get(asset.filename);
    if (!expected) return;
    const actual = await sha256File(path);
    if (actual !== expected) {
      throw new Error(
        `Checksum mismatch for ${asset.filename}: expected ${expected}, got ${actual}. Delete cache and retry: ${path}`,
      );
    }
  };

  if (!existsSync(archivePath)) {
    console.log(`Downloading ${asset.url} to cache...`);
    await download(asset.url, archivePath);
  } else {
    console.log(`Using cached ${asset.filename}`);
  }
  const size = statSync(archivePath).size;
  if (size < 1_000_000) {
    rmSync(archivePath, { force: true });
    throw new Error(
      `Download incomplete (${String(size)} bytes). Deleted corrupted cache. Retry: yarn tv ffmpeg prepare --profile ${profile}`,
    );
  }
  await verify(archivePath);

  const extractDir = join(destDir, "extract");
  mkdirSync(extractDir, { recursive: true });
  try {
    extractZip(archivePath, extractDir);
    const ffmpegPath = findBinaryInExtracted(extractDir, "ffmpeg");
    if (!ffmpegPath) throw new Error("ffmpeg not found in archive");
    const ffprobePath = findBinaryInExtracted(extractDir, "ffprobe");
    if (!ffprobePath) throw new Error("ffprobe not found in archive");
    copyFileSync(ffmpegPath, ffmpegDest);
    copyFileSync(ffprobePath, ffprobeDest);
  } finally {
    rmSync(extractDir, { recursive: true, force: true });
  }

  console.log(`Prepared ffmpeg and ffprobe for ${profile} (${target})`);
}

function prepareMacOsBinaries(target: string, profile: FfmpegProfile): void {
  const ffmpeg = profileFfmpegPath(ROOT, profile, target);
  const ffprobe = profileFfprobePath(ROOT, profile, target);
  const required =
    profile === "lgpl-vt"
      ? [ffmpeg, ffprobe, ...profileLgplDylibPaths(ROOT, profile)]
      : [ffmpeg, ffprobe];

  if (required.every((path) => existsSync(path))) {
    console.log(`FFmpeg binaries already exist for ${profile} (${target}), skipping`);
    return;
  }

  console.log(`Standalone ${profile} binaries not found; building from source...`);
  const script = join(ROOT, profileBuildScript(profile));
  const r = spawnSync("bash", [script], { stdio: "inherit", cwd: ROOT });
  if (r.status !== 0) {
    throw new Error(`FFmpeg build failed (exit ${String(r.status ?? "unknown")}). See output above.`);
  }
  console.log(`Prepared ffmpeg and ffprobe for ${profile} (${target})`);
}

export async function prepareFfmpeg(profile: FfmpegProfile): Promise<void> {
  const target = getTargetTriple();
  assertProfileSupportedOnTarget(profile, target);
  console.log(`Target: ${target}, ffmpeg-profile: ${profile}`);

  if (target.includes("linux")) {
    throw new Error(
      `Bundled profile ${profile} is not supported on Linux in this project.`,
    );
  }

  if (isWindowsTarget(target)) {
    if (profile !== "gpl") {
      throw new Error("Windows standalone supports only --profile gpl.");
    }
    await prepareBtbN(target, profile);
    return;
  }

  if (isMacOsTarget(target)) {
    prepareMacOsBinaries(target, profile);
    return;
  }

  throw new Error(`Unsupported target: ${target}`);
}

const scriptPath = fileURLToPath(import.meta.url);
const argv1 = process.argv[1];
// eslint-disable-next-line @typescript-eslint/no-unnecessary-condition -- argv1 can be undefined when module is imported
const isMain = argv1 != null && resolve(scriptPath) === resolve(argv1);
if (isMain) {
  const profile = parseProfileFromArgv(process.argv.slice(2));
  prepareFfmpeg(profile).catch((err: unknown) => {
    console.error(err);
    process.exit(1);
  });
}
