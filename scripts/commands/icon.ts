import { tmpdir } from "node:os";
import { join } from "node:path";

import { fs } from "zx";

import { type CommandContext,runCommand } from "../runtime.ts";
import { commandExists } from "../utils.ts";

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
    "AppIcon",
    "--include-all-app-icons",
    "--enable-on-demand-resources",
    "NO",
    "--development-region",
    "en",
    "--target-device",
    "mac",
    "--minimum-deployment-target",
    "26.0",
    "--platform",
    "macosx",
  ];
}

export async function runIconCompileCommand(context: CommandContext): Promise<number> {
  if (process.platform !== "darwin") {
    console.log("actool is only available on macOS; skipping Assets.car generation.");
    return 0;
  }

  if (!(await commandExists("actool"))) {
    console.log("actool not found (requires Xcode 26). Skipping Assets.car generation.");
    return 0;
  }

  const iconSrc = join(context.srcTauriDir, "icon-src", "AppIcon.icon");
  const projectAssetsCar = join(context.srcTauriDir, "icons", "Assets.car");

  if (context.dryRun) {
    console.log(`[dry-run] compile ${iconSrc} with actool and copy to ${projectAssetsCar}`);
    return 0;
  }

  const tempDir = fs.mkdtempSync(join(tmpdir(), "tiny-vid-icon-"));
  const iconPath = join(tempDir, "AppIcon.icon");
  const outputPath = join(tempDir, "out");
  const plistPath = join(outputPath, "assetcatalog_generated_info.plist");

  fs.copySync(iconSrc, iconPath);
  fs.ensureDirSync(outputPath);

  try {
    await runCommand(context, "actool", actoolCompileArgs(iconPath, outputPath, plistPath));

    const outputAssetsCar = join(outputPath, "Assets.car");
    fs.ensureDirSync(join(context.srcTauriDir, "icons"));
    fs.copySync(outputAssetsCar, projectAssetsCar);
    console.log("Generated src-tauri/icons/Assets.car");
  } finally {
    fs.removeSync(tempDir);
  }

  return 0;
}
