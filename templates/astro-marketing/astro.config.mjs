import mdx from "@astrojs/mdx";
import { defineConfig } from "astro/config";

// The workbench dev server proxies the site under /preview/{id}/.
const base = process.env.PUBLIC_BASE ?? "/";

export default defineConfig({
  base,
  integrations: [mdx()],
  vite: {
    // Astro keeps Vite's own base at "/", so without this the HMR socket would connect to
    // the proxy root instead of the /preview/{id}/ prefix.
    server: { hmr: { path: `${base}vite-hmr` } },
  },
});
