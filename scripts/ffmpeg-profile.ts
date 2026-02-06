import { execSync } from "node:child_process";
import { join } from "node:path";

export const FFMPEG_PROFILES = ["gpl", "lgpl-vt"] as const;
export type FfmpegProfile = (typeof FFMPEG_PROFILES)[number];

const LGPL_DYLIBS = [
  "libavcodec.dylib",
  "libavdevice.dylib",
  "libavfilter.dylib",
  "libavformat.dylib",
  "libavutil.dylib",
  "libswresample.dylib",
  "libswscale.dylib",
] as const;

export function getTargetTriple(): string {
  return (
    process.env.CARGO_BUILD_TARGET ??
    process.env.TARGET ??
    execSync("rustc --print host-tuple", { encoding: "utf8" }).trim()
  );
}

export function isWindowsTarget(target: string): boolean {
  return target.includes("windows");
}

export function isMacOsTarget(target: string): boolean {
  return target.includes("darwin");
}

export function profileDirName(profile: FfmpegProfile): string {
  return profile === "lgpl-vt" ? "standalone-lgpl-vt" : "standalone-gpl";
}

export function profileTauriConfig(profile: FfmpegProfile): {
  config: string;
  features?: string[];
} {
  if (profile === "lgpl-vt") {
    return { config: "src-tauri/overrides/standalone-lgpl-vt.json", features: ["lgpl"] };
  }
  return { config: "src-tauri/overrides/standalone-gpl.json" };
}

export function sidecarSuffix(target: string): string {
  const exe = isWindowsTarget(target) ? ".exe" : "";
  return `${target}${exe}`;
}

export function profileBinariesDir(rootDir: string, profile: FfmpegProfile): string {
  return join(rootDir, "src-tauri", "binaries", profileDirName(profile));
}

export function profileFfmpegPath(
  rootDir: string,
  profile: FfmpegProfile,
  target: string,
): string {
  return join(profileBinariesDir(rootDir, profile), `ffmpeg-${sidecarSuffix(target)}`);
}

export function profileFfprobePath(
  rootDir: string,
  profile: FfmpegProfile,
  target: string,
): string {
  return join(profileBinariesDir(rootDir, profile), `ffprobe-${sidecarSuffix(target)}`);
}

export function profileLgplDylibPaths(rootDir: string, profile: FfmpegProfile): string[] {
  if (profile !== "lgpl-vt") return [];
  const dir = profileBinariesDir(rootDir, profile);
  return LGPL_DYLIBS.map((name) => join(dir, name));
}

export function assertProfileSupportedOnTarget(
  profile: FfmpegProfile,
  target: string,
): void {
  if (profile === "lgpl-vt" && !isMacOsTarget(target)) {
    throw new Error(
      `FFmpeg profile ${profile} is macOS-only. Target ${target} is not supported.`
    );
  }
}

export function profilePrereqCommand(
  profile: FfmpegProfile,
  target: string,
): string {
  if (profile === "lgpl-vt") {
    return "yarn tv ffmpeg build --profile lgpl-vt";
  }
  return isWindowsTarget(target)
    ? "yarn tv ffmpeg prepare --profile gpl"
    : "yarn tv ffmpeg build --profile gpl";
}
