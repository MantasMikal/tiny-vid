/// <reference types="electron-vite/node" />

interface ElectronAPI {
  webUtils: {
    getPathForFile: (file: File) => string;
  };
}

declare global {
  interface Window {
    electron: ElectronAPI;
  }
}

declare module '*.css' {
  const content: string
  export default content
}

declare module '*.png' {
  const content: string
  export default content
}

declare module '*.jpg' {
  const content: string
  export default content
}

declare module '*.jpeg' {
  const content: string
  export default content
}

declare module '*.svg' {
  const content: string
  export default content
}

declare module '*.web' {
  const content: string
  export default content
}
