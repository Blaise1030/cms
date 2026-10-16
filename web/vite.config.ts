import path from "node:path";
import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import { VitePWA } from "vite-plugin-pwa";
import { pwaOptions } from "./pwa.config.ts";

// `just desktop-dev` moves both so it can run next to `just dev`; the desktop shell reads
// WORKBENCH_DAEMON_PORT too, to start its daemon where this proxies.
const daemonPort = process.env.WORKBENCH_DAEMON_PORT ?? "7800";
const uiPort = Number(process.env.WORKBENCH_VITE_PORT ?? 5173);

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
    include: ["src/**/*.test.ts", "packages/*/src/**/*.test.ts"],
    // Component tests opt in with `// @vitest-environment happy-dom`; never load iframe URLs.
    environmentOptions: { happyDOM: { settings: { disableIframePageLoading: true } } },
  },
  server: {
    // Fixed so the daemon's dev ready link (`just dev`, --ui-url) always points here.
    port: uiPort,
    strictPort: true,
    proxy: {
      // `ws: true` is required: the terminal lives at `/api/terminal/{id}/ws`.
      // The string form proxies HTTP only, so the panel stays blank in `just dev`.
      "/api": { target: `http://127.0.0.1:${daemonPort}`, ws: true },
      "/ws": { target: `ws://127.0.0.1:${daemonPort}`, ws: true },
      "/preview": `http://127.0.0.1:${daemonPort}`,
    },
  },
});
