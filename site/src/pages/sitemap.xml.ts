import type { APIRoute } from "astro";
import { sitePages } from "../lib/pages";
import { absoluteUrl } from "../lib/seo";

export const prerender = true;

const escape = (value: string) => value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

/** Every page of the site, built from src/pages and the Blog and Help collections. */
export const GET: APIRoute = async () => {
  const urls = (await sitePages()).map(
    (page) => `  <url>\n    <loc>${escape(absoluteUrl(page.path))}</loc>${page.lastmod ? `\n    <lastmod>${page.lastmod}</lastmod>` : ""}\n  </url>`,
  );
  const xml = `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls.join("\n")}\n</urlset>\n`;
  return new Response(xml, { headers: { "Content-Type": "application/xml; charset=utf-8" } });
};
