import path from "node:path";
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import stylex from "@stylexjs/unplugin/vite";

const daemonPort = process.env.ORSO_DAEMON_PORT ?? "7800";
const uiPort = Number(process.env.ORSO_VITE_PORT ?? 5174);

export default defineConfig(({ command }) => {
  const development = command === "serve";
  // Lenso ships StyleX declarations — must be transformed by the plugin, never prebundled.
  const lensoStylexPackages = ["@lenso/ui", "@lenso/tokens", "@lenso/glass", "@lenso/primitives"];
  return {
    plugins: [
      stylex({
        // @ts-expect-error unplugin accepts this; types omit it from UserOptions
        externalPackages: lensoStylexPackages,
        dev: development,
        runtimeInjection: false,
        useCSSLayers: false,
        unstable_moduleResolution: { type: "commonJS" },
        devMode: "full",
      }),
      react(),
    ],
    resolve: {
      alias: {
        "@": path.resolve(import.meta.dirname, "./src"),
      },
      dedupe: ["react", "react-dom", "@stylexjs/stylex"],
    },
    optimizeDeps: {
      include: ["react", "react-dom/client", "react/jsx-runtime", "react/jsx-dev-runtime"],
      exclude: lensoStylexPackages,
    },
    test: {
      environment: "node",
      include: ["src/**/*.test.ts"],
    },
    server: {
      port: uiPort,
      strictPort: true,
      proxy: {
        "/api": { target: `http://127.0.0.1:${daemonPort}`, ws: true },
        "/ws": { target: `ws://127.0.0.1:${daemonPort}`, ws: true },
        "/preview": `http://127.0.0.1:${daemonPort}`,
      },
    },
  };
});
