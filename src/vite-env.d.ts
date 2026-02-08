/// <reference types="vite/client" />

import type { TinyVidElectronBridge } from "@/platform/desktop/bridge";

declare global {
  interface Window {
    __TINY_VID_ELECTRON__?: TinyVidElectronBridge;
  }
}

export {};
