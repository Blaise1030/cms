import mdx from "@astrojs/mdx";
import { defineConfig } from "astro/config";

export default defineConfig({
  // The workbench dev server proxies the site under /preview/{id}/.
  base: process.env.PUBLIC_BASE ?? "/",
  integrations: [mdx()],
});
