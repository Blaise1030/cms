import path from "node:path";
import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import { VitePWA } from "vite-plugin-pwa";
import { pwaOptions } from "./pwa.config.ts";

export default defineConfig({
  plugins: [vue(), tailwindcss(), VitePWA(pwaOptions)],
  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "./src"),
    },
  },
  test: {
    environment: "node",
    setupFiles: ["src/test/vitest-globals.ts"],
    include: ["src/**/*.test.ts"],
    // Component tests opt in with `// @vitest-environment happy-dom`; never load iframe URLs.
    environmentOptions: { happyDOM: { settings: { disableIframePageLoading: true } } },
  },
  server: {
    // Fixed so the daemon's dev ready link (`just dev`, --ui-url) always points here.
    port: 5173,
    strictPort: true,
    proxy: {
      "/api": "http://127.0.0.1:7800",
      "/ws": { target: "ws://127.0.0.1:7800", ws: true },
      "/preview": "http://127.0.0.1:7800",
    },
  },
});
