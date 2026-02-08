import type {
  DesktopOpenDialogOptions,
  DesktopPlatform,
  DesktopSaveDialogOptions,
  TinyVidElectronBridge,
} from "@/platform/desktop/bridge";
import type { NativeInvokeArgs, NativeInvokeCommand, NativeInvokeResult } from "@/types/native";

type Listener<T> = (payload: T) => void;
type Unlisten = () => void;

type InvokeArgsTuple<C extends NativeInvokeCommand> =
  undefined extends NativeInvokeArgs<C> ? [] | [NativeInvokeArgs<C>] : [NativeInvokeArgs<C>];

function getElectronBridge(command?: string): TinyVidElectronBridge {
  if (typeof window === "undefined") {
    throw new Error("Tiny Vid desktop commands are unavailable in non-browser runtime");
  }

  const bridge = window.__TINY_VID_ELECTRON__;
  if (bridge) {
    return bridge;
  }

  if (command) {
    throw new Error(
      `Desktop command "${command}" is unavailable because Electron preload bridge did not initialize`,
    );
  }

  throw new Error("Electron preload bridge did not initialize");
}

export const desktopClient = {
  async invoke<C extends NativeInvokeCommand>(
    command: C,
    ...argsTuple: InvokeArgsTuple<C>
  ): Promise<NativeInvokeResult<C>> {
    const bridge = getElectronBridge(command);
    const args = argsTuple[0];
    return (await bridge.invoke({ command, ...(args === undefined ? {} : { args }) })) as NativeInvokeResult<C>;
  },

  listen<T>(event: string, handler: Listener<T>): Promise<Unlisten> {
    const bridge = getElectronBridge();
    return Promise.resolve(
      bridge.on(event, (payload) => {
        handler(payload as T);
      }),
    );
  },

  async openDialog(options: DesktopOpenDialogOptions): Promise<string | string[] | null> {
    const bridge = getElectronBridge();
    return await bridge.openDialog(options);
  },

  async saveDialog(options: DesktopSaveDialogOptions): Promise<string | null> {
    const bridge = getElectronBridge();
    return await bridge.saveDialog(options);
  },

  async platform(): Promise<DesktopPlatform> {
    const bridge = getElectronBridge();
    return await bridge.platform();
  },

  async toMediaSrc(path: string): Promise<string> {
    const bridge = getElectronBridge();
    return await bridge.toMediaSrc(path);
  },

  pathForFile(file: File): Promise<string | null> {
    const bridge = getElectronBridge();
    const path = bridge.pathForFile(file);
    return Promise.resolve(path.length > 0 ? path : null);
  },
};
