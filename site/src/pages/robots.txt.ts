import type { APIRoute } from "astro";
import { absoluteUrl } from "../lib/seo";

export const prerender = true;

/** Allow every crawler and point it at the sitemap. */
export const GET: APIRoute = () =>
  new Response(`User-agent: *\nAllow: /\n\nSitemap: ${absoluteUrl("/sitemap.xml")}\n`, {
    headers: { "Content-Type": "text/plain; charset=utf-8" },
  });
