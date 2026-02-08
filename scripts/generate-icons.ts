import { execFile } from "node:child_process";
import { access, copyFile, cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

import { Icns, IcnsImage } from "@fiahfy/icns";
import pngToIco from "png-to-ico";
import sharp from "sharp";

import { formatError } from "./utils.ts";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const ROOT_DIR = join(__dirname, "..");
const SOURCE_ICON = join(ROOT_DIR, "app-icon.png");
const OUTPUT_DIR = join(ROOT_DIR, "electron", "icons");
const MACOS_ASSETS_CAR_OUTPUT = join(ROOT_DIR, "electron", "resources", "macos", "Assets.car");
const APP_ICON_NAME = "AppIcon";
const APP_ICON_BUNDLE_PATH = join(ROOT_DIR, "electron", "icon-src", `${APP_ICON_NAME}.icon`);
const ACTOOL_MIN_DEPLOYMENT_TARGET = "26.0";

const execFileAsync = promisify(execFile);

const ICNS_VARIANTS = [
  { size: 16, type: "icp4" },
  { size: 32, type: "icp5" },
  { size: 64, type: "icp6" },
  { size: 128, type: "ic07" },
  { size: 256, type: "ic08" },
  { size: 512, type: "ic09" },
  { size: 1024, type: "ic10" },
] as const;

const ICO_SIZES = [16, 24, 32, 48, 64, 128, 256] as const;

function parseFlags(argv: string[]): { dryRun: boolean } {
  let dryRun = false;

  for (const arg of argv) {
    if (arg === "--dry-run") {
      dryRun = true;
      continue;
    }
    if (arg === "--help" || arg === "-h") {
      console.log("Usage: node scripts/generate-icons.ts [--dry-run]");
      process.exit(0);
    }
    throw new Error(`Unknown argument: ${arg}`);
  }

  return { dryRun };
}

async function renderPng(size: number): Promise<Buffer> {
  return sharp(SOURCE_ICON)
    .resize(size, size, {
      fit: "contain",
      background: { r: 0, g: 0, b: 0, alpha: 0 },
    })
    .png({ compressionLevel: 9 })
    .toBuffer();
}

async function generateIco(iconPath: string): Promise<void> {
  const tempDir = await mkdtemp(join(tmpdir(), "tiny-vid-ico-"));
  try {
    const pngPaths: string[] = [];
    for (const size of ICO_SIZES) {
      const out = join(tempDir, `${String(size)}.png`);
      const png = await renderPng(size);
      await writeFile(out, png);
      pngPaths.push(out);
    }

    const ico = await pngToIco(pngPaths);
    await writeFile(iconPath, ico);
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
}

async function generateIcns(iconPath: string): Promise<void> {
  const icns = new Icns();
  for (const variant of ICNS_VARIANTS) {
    const png = await renderPng(variant.size);
    icns.append(IcnsImage.fromPNG(png, variant.type));
  }
  await writeFile(iconPath, icns.data);
}

async function assertSourceIconReadable(): Promise<void> {
  const source = await readFile(SOURCE_ICON);
  const metadata = await sharp(source).metadata();
  if (!metadata.width || !metadata.height) {
    throw new Error(`Unable to determine icon dimensions for ${SOURCE_ICON}`);
  }
  if (metadata.width !== metadata.height) {
    throw new Error(
      `Icon source must be square. Got ${String(metadata.width)}x${String(metadata.height)}`
    );
  }
}

async function pathExists(pathToCheck: string): Promise<boolean> {
  try {
    await access(pathToCheck);
    return true;
  } catch {
    return false;
  }
}

async function findAppIconBundlePath(): Promise<string | null> {
  if (await pathExists(APP_ICON_BUNDLE_PATH)) {
    return APP_ICON_BUNDLE_PATH;
  }
  return null;
}

async function resolveActoolPath(): Promise<string | null> {
  try {
    const { stdout } = await execFileAsync("xcrun", ["--find", "actool"]);
    const path = stdout.trim();
    if (path.length === 0) {
      return null;
    }
    return path;
  } catch {
    return null;
  }
}

function actoolCompileArgs(iconPath: string, outputPath: string, plistPath: string): string[] {
  return [
    iconPath,
    "--compile",
    outputPath,
    "--output-format",
    "human-readable-text",
    "--notices",
    "--warnings",
    "--errors",
    "--output-partial-info-plist",
    plistPath,
    "--app-icon",
    APP_ICON_NAME,
    "--include-all-app-icons",
    "--enable-on-demand-resources",
    "NO",
    "--development-region",
    "en",
    "--target-device",
    "mac",
    "--minimum-deployment-target",
    ACTOOL_MIN_DEPLOYMENT_TARGET,
    "--platform",
    "macosx",
  ];
}

async function generateMacOsTahoeArtifacts(
  actoolPath: string,
  appIconBundlePath: string,
  iconPath: string
): Promise<void> {
  const tempDir = await mkdtemp(join(tmpdir(), "tiny-vid-appicon-"));
  try {
    const appIconPath = join(tempDir, `${APP_ICON_NAME}.icon`);
    const outputPath = join(tempDir, "out");
    const plistPath = join(outputPath, "assetcatalog_generated_info.plist");
    const outputIcns = join(outputPath, `${APP_ICON_NAME}.icns`);
    const outputAssetsCar = join(outputPath, "Assets.car");

    await cp(appIconBundlePath, appIconPath, { recursive: true });
    await mkdir(outputPath, { recursive: true });

    try {
      const { stdout, stderr } = await execFileAsync(
        actoolPath,
        actoolCompileArgs(appIconPath, outputPath, plistPath),
        {
          maxBuffer: 1024 * 1024 * 16,
        }
      );
      if (stdout.trim().length > 0) {
        process.stdout.write(stdout);
      }
      if (stderr.trim().length > 0) {
        process.stderr.write(stderr);
      }
    } catch (error) {
      throw new Error(`actool failed while compiling ${normalize(appIconBundlePath)}: ${formatError(error)}`, {
        cause: error,
      });
    }

    await copyFile(outputIcns, iconPath);
    await mkdir(dirname(MACOS_ASSETS_CAR_OUTPUT), { recursive: true });
    await copyFile(outputAssetsCar, MACOS_ASSETS_CAR_OUTPUT);
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
}

async function generateMacIcns(iconPath: string): Promise<void> {
  const appIconBundlePath = await findAppIconBundlePath();
  if (!appIconBundlePath) {
    console.log("AppIcon.icon source not found; falling back to PNG-based macOS icon generation.");
    await generateIcns(iconPath);
    return;
  }

  const actoolPath = await resolveActoolPath();
  if (!actoolPath) {
    console.log("actool not found; falling back to PNG-based macOS icon generation.");
    await generateIcns(iconPath);
    return;
  }

  await generateMacOsTahoeArtifacts(actoolPath, appIconBundlePath, iconPath);
}

async function main(): Promise<void> {
  const { dryRun } = parseFlags(process.argv.slice(2));
  const outPng = join(OUTPUT_DIR, "icon.png");
  const outIco = join(OUTPUT_DIR, "icon.ico");
  const outIcns = join(OUTPUT_DIR, "icon.icns");

  if (dryRun) {
    console.log(`[dry-run] read ${SOURCE_ICON}`);
    console.log(`[dry-run] write ${outPng}`);
    console.log(`[dry-run] write ${outIco}`);
    console.log(`[dry-run] write ${outIcns}`);
    if (process.platform === "darwin") {
      console.log(
        `[dry-run] write ${MACOS_ASSETS_CAR_OUTPUT} (when AppIcon.icon + actool are available)`
      );
    }
    return;
  }

  await assertSourceIconReadable();

  const png512 = await renderPng(512);
  await writeFile(outPng, png512);
  await generateIco(outIco);
  if (process.platform === "darwin") {
    await generateMacIcns(outIcns);
  } else {
    await generateIcns(outIcns);
  }

  console.log("Generated icons:");
  console.log(`- ${outPng}`);
  console.log(`- ${outIco}`);
  console.log(`- ${outIcns}`);
  if (process.platform === "darwin" && (await pathExists(MACOS_ASSETS_CAR_OUTPUT))) {
    console.log(`- ${MACOS_ASSETS_CAR_OUTPUT}`);
  }
}

main().catch((error: unknown) => {
  console.error(error);
  process.exit(1);
});
