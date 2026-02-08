import type { NativeInvokeArgs, NativeInvokeCommand } from "@/types/native";

export interface DesktopDialogFilter {
  name: string;
  extensions: string[];
}

export interface DesktopOpenDialogOptions {
  multiple?: boolean;
  directory?: boolean;
  filters?: DesktopDialogFilter[];
}

export interface DesktopSaveDialogOptions {
  defaultPath?: string;
  filters?: DesktopDialogFilter[];
}

export type DesktopPlatform = string;

export interface ElectronInvokeRequest<C extends NativeInvokeCommand = NativeInvokeCommand> {
  command: C;
  args?: NativeInvokeArgs<C>;
}

export interface TinyVidElectronBridge {
  invoke<C extends NativeInvokeCommand>(request: ElectronInvokeRequest<C>): Promise<unknown>;
  on(event: string, handler: (payload: unknown) => void): () => void;
  openDialog(options: DesktopOpenDialogOptions): Promise<string | string[] | null>;
  saveDialog(options: DesktopSaveDialogOptions): Promise<string | null>;
  platform(): Promise<DesktopPlatform>;
  toMediaSrc(path: string): Promise<string>;
  pathForFile(file: File): string;
}
