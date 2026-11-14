import path from "node:path";
import { fileURLToPath } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import vue from "@vitejs/plugin-vue";
import type { Plugin } from "vite";
import { defineConfig } from "vite";

const root = path.dirname(fileURLToPath(import.meta.url));

/** Bundle emitted CSS into the JS entry as `__ORSO_CHECKS_CSS__` for Shadow DOM. */
function cssToShadowString(): Plugin {
  return {
    name: "orso-checks-css-to-shadow",
    apply: "build",
    enforce: "post",
    generateBundle(_options, bundle) {
      let css = "";
      for (const [fileName, item] of Object.entries(bundle)) {
        if (item.type === "asset" && fileName.endsWith(".css")) {
          css += typeof item.source === "string" ? item.source : new TextDecoder().decode(item.source);
          delete bundle[fileName];
        }
      }
      for (const item of Object.values(bundle)) {
        if (item.type === "chunk" && item.isEntry) {
          item.code = `const __ORSO_CHECKS_CSS__ = ${JSON.stringify(css)};\n${item.code}`;
        }
      }
    },
  };
}

export default defineConfig({
  plugins: [vue(), tailwindcss(), cssToShadowString()],
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
