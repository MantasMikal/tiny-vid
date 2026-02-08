import { resolve } from "node:path";

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const host = process.env.TINY_VID_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(() => ({
  plugins: [
    react({
      babel: {
        plugins: [["babel-plugin-react-compiler", {}]],
      },
    }),
    tailwindcss(),
  ],
  resolve: {
    alias: {
      "@": resolve(import.meta.dirname, "./src"),
    },
  },

  // Vite options tailored for desktop development.
  //
  // 1. prevent vite from obscuring native-sidecar errors
  clearScreen: false,
  // 2. keep a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host ?? false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell vite to ignore watching `native`
      ignored: ["**/native/**"],
    },
  },
}));
