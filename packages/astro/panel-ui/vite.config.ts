import path from "node:path";
import { fileURLToPath } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vite";
import cssInjectedByJs from "vite-plugin-css-injected-by-js";

const root = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [vue(), tailwindcss(), cssInjectedByJs()],
  build: {
    outDir: path.resolve(root, "../src/panel"),
    emptyOutDir: false,
    lib: {
      entry: path.resolve(root, "src/main.ts"),
      formats: ["es"],
      fileName: () => "checks-panel.js",
    },
    rollupOptions: {
      output: {
        inlineDynamicImports: true,
      },
    },
    cssCodeSplit: false,
    minify: true,
    target: "es2022",
  },
  define: {
    "process.env.NODE_ENV": JSON.stringify("production"),
  },
});
