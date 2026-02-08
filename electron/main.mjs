import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import readline from "node:readline";
import { fileURLToPath, pathToFileURL } from "node:url";

import { app, BrowserWindow, Menu, dialog, ipcMain, net, protocol } from "electron";

const IS_VM = !!(
  process.env.IS_VM ||
  process.env.TINY_VID_IS_VM
);

// Targeted fix for Linux VMs (Parallels, etc.): sandbox, /dev/shm, GPU issues (Electron #26061)
if (process.platform === "linux" && IS_VM) {
  app.commandLine.appendSwitch("no-sandbox");
  app.commandLine.appendSwitch("disable-dev-shm-usage");
  app.commandLine.appendSwitch("disable-seccomp-filter-sandbox");
}

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const ROOT_DIR = path.resolve(__dirname, "..");
const DIST_DIR = path.join(ROOT_DIR, "dist");
const PRELOAD_PATH = path.join(__dirname, "preload.cjs");
const DEV_SERVER_URL = process.env.TINY_VID_DEV_SERVER_URL ?? process.env.VITE_DEV_SERVER_URL;
const MEDIA_PROTOCOL = "tiny-vid-media";
const MEDIA_PROTOCOL_HOST = "local";

protocol.registerSchemesAsPrivileged([
  {
    scheme: MEDIA_PROTOCOL,
    privileges: {
      standard: true,
      secure: true,
      stream: true,
    },
  },
]);

let mainWindow = null;
let pendingOpenedFiles = [];

let sidecarProcess = null;
let sidecarReadline = null;
let sidecarNextRequestId = 1;
const sidecarPendingRequests = new Map();

function normalizePlatform() {
  switch (process.platform) {
    case "darwin":
      return "macos";
    case "win32":
      return "windows";
    default:
      return process.platform;
  }
}

function emitDesktopEvent(event, payload) {
  for (const window of BrowserWindow.getAllWindows()) {
    window.webContents.send("tiny-vid:event", { event, payload });
  }
}

function bufferOpenedFiles(paths) {
  if (!Array.isArray(paths) || paths.length === 0) {
    return;
  }
  pendingOpenedFiles.push(...paths);
  emitDesktopEvent("open-file", paths);
}

function parseLaunchPaths(argv) {
  const startIndex = app.isPackaged ? 1 : 2;
  const paths = [];
  const mainScriptPath = path.resolve(__dirname, "main.mjs");

  for (const rawArg of argv.slice(startIndex)) {
    if (!rawArg || rawArg.startsWith("-")) {
      continue;
    }
    try {
      const parsed = new URL(rawArg);
      if (parsed.protocol === "file:") {
        const resolved = fileURLToPath(parsed);
        if (resolved !== mainScriptPath) paths.push(resolved);
        continue;
      }
    } catch {
      // Not a URL, continue with raw path.
    }
    const resolved = path.resolve(rawArg);
    if (resolved !== mainScriptPath) paths.push(resolved);
  }

  return [...new Set(paths)];
}

function toMediaProtocolUrl(inputPath) {
  return `${MEDIA_PROTOCOL}://${MEDIA_PROTOCOL_HOST}/?path=${encodeURIComponent(inputPath)}`;
}

async function resolveMediaPath(inputPath) {
  if (typeof inputPath !== "string" || inputPath.length === 0) {
    throw new Error("Missing media path");
  }
  if (!path.isAbsolute(inputPath)) {
    throw new Error("Media path must be absolute");
  }

  const resolvedPath = await fs.promises.realpath(inputPath);
  const stat = await fs.promises.stat(resolvedPath);
  if (!stat.isFile()) {
    throw new Error("Media path is not a file");
  }
  return resolvedPath;
}

function registerMediaProtocol() {
  protocol.handle(MEDIA_PROTOCOL, async (request) => {
    let mediaPath = null;
    try {
      const requestUrl = new URL(request.url);
      if (requestUrl.hostname !== MEDIA_PROTOCOL_HOST) {
        return new Response("Invalid media host", { status: 400 });
      }
      mediaPath = requestUrl.searchParams.get("path");
    } catch {
      return new Response("Invalid media URL", { status: 400 });
    }

    if (!mediaPath || typeof mediaPath !== "string") {
      return new Response("Missing media path", { status: 400 });
    }

    try {
      const resolvedPath = await resolveMediaPath(mediaPath);
      return net.fetch(pathToFileURL(resolvedPath).toString());
    } catch {
      return new Response("Unable to open media file", { status: 404 });
    }
  });
}

function createApplicationMenu() {
  if (process.platform !== "darwin") {
    Menu.setApplicationMenu(null);
    return;
  }

  const template = [
    { role: "appMenu" },
    {
      label: "File",
      submenu: [
        {
          label: "Open File",
          accelerator: "CmdOrCtrl+O",
          click: () => {
            emitDesktopEvent("menu-open-file", null);
          },
        },
        { type: "separator" },
        { role: "quit" },
      ],
    },
    {
      label: "View",
      submenu: [{ role: "reload" }, { role: "toggledevtools" }, { role: "togglefullscreen" }],
    },
    {
      label: "Window",
      submenu: [{ role: "minimize" }, { role: "close" }],
    },
  ];

  const menu = Menu.buildFromTemplate(template);
  Menu.setApplicationMenu(menu);
}

function createMainWindow() {
  const isMac = process.platform === "darwin";
  const window = new BrowserWindow({
    title: "Tiny Vid",
    width: 900,
    height: 670,
    minWidth: 375,
    minHeight: 640,
    show: false,
    backgroundColor: "#0a0a0a",
    ...(isMac ? { titleBarStyle: "hiddenInset" } : {}),
    webPreferences: {
      preload: PRELOAD_PATH,
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });

  window.once("ready-to-show", () => {
    window.show();
  });

  // On Linux VMs, ready-to-show may never fire if GPU rendering fails. Fallback: show after 3s.
  if (process.platform === "linux" && IS_VM) {
    setTimeout(() => {
      if (!window.isDestroyed() && !window.isVisible()) {
        window.show();
      }
    }, 3000);
  }

  if (DEV_SERVER_URL) {
    void window.loadURL(DEV_SERVER_URL);
  } else {
    void window.loadFile(path.join(DIST_DIR, "index.html"));
  }

  return window;
}

function sidecarRejectAllPending(reason) {
  for (const request of sidecarPendingRequests.values()) {
    request.reject(reason);
  }
  sidecarPendingRequests.clear();
}

function handleSidecarMessage(rawLine) {
  if (!rawLine || rawLine.trim().length === 0) {
    return;
  }

  let message;
  try {
    message = JSON.parse(rawLine);
  } catch {
    console.warn("[sidecar] failed to parse line:", rawLine);
    return;
  }

  if (message?.event && typeof message.event === "string") {
    emitDesktopEvent(message.event, message.payload ?? null);
    return;
  }

  const id = message?.id;
  if (typeof id !== "number") {
    return;
  }

  const pending = sidecarPendingRequests.get(id);
  if (!pending) {
    return;
  }
  sidecarPendingRequests.delete(id);

  if (Object.prototype.hasOwnProperty.call(message, "error")) {
    pending.reject(message.error);
    return;
  }
  pending.resolve(message.result);
}

function resolveSidecarLaunch() {
  const explicitSidecarPath = process.env.TINY_VID_SIDECAR_PATH;
  if (explicitSidecarPath && explicitSidecarPath.trim().length > 0) {
    return {
      command: explicitSidecarPath,
      args: [],
      cwd: ROOT_DIR,
    };
  }

  const sidecarExecutableName = process.platform === "win32" ? "tiny-vid-sidecar.exe" : "tiny-vid-sidecar";

  if (app.isPackaged) {
    const sidecarPath = path.join(process.resourcesPath, "bin", sidecarExecutableName);
    return {
      command: sidecarPath,
      args: [],
      cwd: process.resourcesPath,
    };
  }

  const debugSidecarPath = path.join(ROOT_DIR, "native", "target", "debug", sidecarExecutableName);
  if (fs.existsSync(debugSidecarPath)) {
    return {
      command: debugSidecarPath,
      args: [],
      cwd: ROOT_DIR,
    };
  }

  const manifestPath = path.join(ROOT_DIR, "native", "Cargo.toml");
  return {
    command: "cargo",
    args: ["run", "--manifest-path", manifestPath, "--bin", "tiny-vid-sidecar", "--quiet"],
    cwd: ROOT_DIR,
  };
}

async function ensureSidecarRunning() {
  if (sidecarProcess && !sidecarProcess.killed) {
    return;
  }

  const launch = resolveSidecarLaunch();
  sidecarProcess = spawn(launch.command, launch.args, {
    cwd: launch.cwd,
    stdio: ["pipe", "pipe", "pipe"],
    env: process.env,
  });

  sidecarProcess.stdout.setEncoding("utf8");
  sidecarProcess.stderr.setEncoding("utf8");

  sidecarReadline = readline.createInterface({ input: sidecarProcess.stdout });
  sidecarReadline.on("line", handleSidecarMessage);

  sidecarProcess.stderr.on("data", (chunk) => {
    const text = String(chunk).trim();
    if (text.length > 0) {
      console.error("[sidecar]", text);
    }
  });

  sidecarProcess.on("exit", (code, signal) => {
    const reason = {
      summary: "Sidecar exited",
      detail: `Sidecar exited (code=${String(code)}, signal=${String(signal)})`,
    };
    sidecarRejectAllPending(reason);
    sidecarProcess = null;
    if (sidecarReadline) {
      sidecarReadline.close();
      sidecarReadline = null;
    }
  });

  sidecarProcess.on("error", (err) => {
    const reason = {
      summary: "Sidecar launch failed",
      detail: err instanceof Error ? err.message : String(err),
    };
    sidecarRejectAllPending(reason);
    sidecarProcess = null;
    if (sidecarReadline) {
      sidecarReadline.close();
      sidecarReadline = null;
    }
  });

  await sidecarRequest("app.capabilities", {});
}

function sidecarRequest(method, params = {}) {
  return new Promise((resolve, reject) => {
    if (!sidecarProcess || sidecarProcess.killed || !sidecarProcess.stdin.writable) {
      reject({
        summary: "Sidecar not available",
        detail: "Sidecar process is not running",
      });
      return;
    }

    const id = sidecarNextRequestId++;
    sidecarPendingRequests.set(id, { resolve, reject });

    const payload = JSON.stringify({
      id,
      method,
      params,
    });

    sidecarProcess.stdin.write(`${payload}\n`, (err) => {
      if (!err) return;
      sidecarPendingRequests.delete(id);
      reject({
        summary: "Sidecar write failed",
        detail: err.message,
      });
    });
  });
}

function stopSidecar() {
  if (!sidecarProcess) {
    return;
  }
  if (sidecarReadline) {
    sidecarReadline.close();
    sidecarReadline = null;
  }
  sidecarProcess.kill();
  sidecarProcess = null;
  sidecarRejectAllPending({
    summary: "Sidecar stopped",
    detail: "Sidecar process has stopped",
  });
}

function registerIpcHandlers() {
  ipcMain.handle("tiny-vid:invoke", async (_event, request) => {
    const command = request?.command;
    const args = request?.args ?? {};

    if (typeof command !== "string" || command.length === 0) {
      throw new Error("Invalid command");
    }

    if (command === "get_pending_opened_files") {
      const result = [...pendingOpenedFiles];
      pendingOpenedFiles = [];
      return result;
    }

    await ensureSidecarRunning();

    try {
      return await sidecarRequest(command, args);
    } catch (err) {
      const detail =
        err && typeof err === "object" && "detail" in err ? String(err.detail) : String(err);
      const summary =
        err && typeof err === "object" && "summary" in err ? String(err.summary) : "Sidecar Error";
      throw new Error(JSON.stringify({ summary, detail }));
    }
  });

  ipcMain.handle("tiny-vid:dialog:open", async (event, options = {}) => {
    const ownerWindow = BrowserWindow.fromWebContents(event.sender) ?? mainWindow ?? undefined;
    const properties = [];
    if (options.directory) properties.push("openDirectory");
    if (!options.directory) properties.push("openFile");
    if (options.multiple) properties.push("multiSelections");

    const result = await dialog.showOpenDialog(ownerWindow, {
      filters: options.filters ?? [],
      properties,
    });

    if (result.canceled) return null;
    if (options.multiple) return result.filePaths;
    return result.filePaths[0] ?? null;
  });

  ipcMain.handle("tiny-vid:dialog:save", async (event, options = {}) => {
    const ownerWindow = BrowserWindow.fromWebContents(event.sender) ?? mainWindow ?? undefined;
    const result = await dialog.showSaveDialog(ownerWindow, {
      defaultPath: options.defaultPath,
      filters: options.filters ?? [],
    });
    if (result.canceled) return null;
    return result.filePath ?? null;
  });

  ipcMain.handle("tiny-vid:platform", async () => normalizePlatform());

  ipcMain.handle("tiny-vid:to-media-src", async (_event, inputPath) => {
    const resolvedPath = await resolveMediaPath(inputPath);
    return toMediaProtocolUrl(resolvedPath);
  });
}

if (!app.requestSingleInstanceLock()) {
  app.quit();
} else {
  app.on("second-instance", (_event, argv) => {
    const openPaths = parseLaunchPaths(argv);
    if (openPaths.length > 0) {
      bufferOpenedFiles(openPaths);
    }

    if (mainWindow) {
      if (mainWindow.isMinimized()) mainWindow.restore();
      mainWindow.focus();
    }
  });
}

app.on("open-file", (event, inputPath) => {
  event.preventDefault();
  if (typeof inputPath === "string" && inputPath.length > 0) {
    bufferOpenedFiles([inputPath]);
  }
});

app.whenReady().then(async () => {
  registerMediaProtocol();
  registerIpcHandlers();
  createApplicationMenu();
  mainWindow = createMainWindow();

  const openPaths = parseLaunchPaths(process.argv);
  if (openPaths.length > 0) {
    bufferOpenedFiles(openPaths);
  }

  try {
    await ensureSidecarRunning();
  } catch (err) {
    console.error("[sidecar] failed to start on app boot:", err);
  }
});

app.on("activate", () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    mainWindow = createMainWindow();
  }
});

app.on("before-quit", () => {
  stopSidecar();
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});
