import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const ROOT_DIR = path.resolve(__dirname, "..", "..");
const OUTPUT_DIR = path.join(ROOT_DIR, "releases", "electron");
const SIDECAR_NAMES = new Set(["tiny-vid-sidecar", "tiny-vid-sidecar.exe"]);

function walk(dir: string, files: string[] = []): string[] {
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      walk(fullPath, files);
      continue;
    }
    files.push(fullPath);
  }
  return files;
}

function toPosix(filePath: string): string {
  return filePath.split(path.sep).join("/");
}

function main(): void {
  if (!fs.existsSync(OUTPUT_DIR)) {
    throw new Error(`Electron output directory does not exist: ${OUTPUT_DIR}`);
  }

  const allFiles = walk(OUTPUT_DIR);
  const sidecarCandidates = allFiles.filter((filePath) => {
    if (!SIDECAR_NAMES.has(path.basename(filePath))) {
      return false;
    }
    const posixPath = toPosix(filePath);
    return posixPath.includes("/resources/bin/") || posixPath.includes("/Resources/bin/");
  });

  if (sidecarCandidates.length === 0) {
    throw new Error(`No bundled sidecar found under resources/bin in ${OUTPUT_DIR}`);
  }

  console.log("Bundled sidecar detected:");
  for (const candidate of sidecarCandidates) {
    console.log(`- ${candidate}`);
  }
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
