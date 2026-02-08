import type { ChildProcess } from "node:child_process";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";

import { runInheritedCommand, spawnInherited } from "../process.ts";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const ROOT_DIR = path.resolve(__dirname, "..", "..");

const VITE_PORT = Number(process.env.TINY_VID_DEV_PORT ?? 1420);
const DEV_SERVER_URL = process.env.TINY_VID_DEV_SERVER_URL ?? `http://127.0.0.1:${String(VITE_PORT)}`;
const SIDECAR_BINARY = process.platform === "win32" ? "tiny-vid-sidecar.exe" : "tiny-vid-sidecar";
const SIDECAR_PATH = path.join(ROOT_DIR, "native", "target", "debug", SIDECAR_BINARY);
const YARN_CMD = process.platform === "win32" ? "yarn.cmd" : "yarn";

async function waitForServer(url: string, timeoutMs = 60_000): Promise<void> {
  const start = Date.now();
  for (;;) {
    try {
      const response = await fetch(url, { method: "GET" });
      if (response.ok || response.status < 500) {
        return;
      }
    } catch {
      // Retry until timeout.
    }

    if (Date.now() - start > timeoutMs) {
      throw new Error(`Timed out waiting for dev server at ${url}`);
    }
    await delay(250);
  }
}

function terminate(child: ChildProcess | null): void {
  if (!child || child.killed) {
    return;
  }
  child.kill("SIGTERM");
  setTimeout(() => {
    if (!child.killed) {
      child.kill("SIGKILL");
    }
  }, 2_000);
}

let shuttingDown = false;
let viteProcess: ChildProcess | null = null;
let electronProcess: ChildProcess | null = null;

function shutdown(exitCode = 0): void {
  if (shuttingDown) {
    return;
  }
  shuttingDown = true;
  terminate(electronProcess);
  terminate(viteProcess);
  process.exitCode = exitCode;
}

process.on("SIGINT", () => shutdown(0));
process.on("SIGTERM", () => shutdown(0));

async function main(): Promise<void> {
  const launchedVite = spawnInherited(
    YARN_CMD,
    ["dev:web", "--host", "127.0.0.1", "--port", String(VITE_PORT), "--strictPort"],
    {
      cwd: ROOT_DIR,
      env: process.env,
    },
  );
  viteProcess = launchedVite;

  launchedVite.on("exit", (code) => {
    if (!shuttingDown) {
      shutdown(code ?? 1);
    }
  });

  await waitForServer(DEV_SERVER_URL);

  await runInheritedCommand(
    "cargo",
    ["build", "--manifest-path", path.join(ROOT_DIR, "native", "Cargo.toml"), "--bin", "tiny-vid-sidecar"],
    {
      cwd: ROOT_DIR,
      env: process.env,
    },
  );

  const launchedElectron = spawnInherited(YARN_CMD, ["electron:shell"], {
    cwd: ROOT_DIR,
    env: {
      ...process.env,
      TINY_VID_DEV_SERVER_URL: DEV_SERVER_URL,
      TINY_VID_SIDECAR_PATH: SIDECAR_PATH,
    },
  });
  electronProcess = launchedElectron;

  await new Promise<void>((resolve, reject) => {
    launchedElectron.on("error", (error) => {
      reject(error);
    });
    launchedElectron.on("exit", (code) => {
      shutdown(code ?? 0);
      resolve();
    });
  });
}

try {
  await main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  shutdown(1);
}
