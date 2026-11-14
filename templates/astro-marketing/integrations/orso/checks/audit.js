import { readdir, readFile } from "node:fs/promises";
import { join, relative } from "node:path";
import { parseHTML } from "linkedom";

import { auditPage } from "./page.js";
import { auditSite } from "./site.js";

/**
 * @param {string} dir
 * @param {{ base?: string }} [opts]
 * @returns {Promise<{ findings: import("./findings.js").Finding[], pages: { path: string }[] }>}
 */
export async function auditDist(dir, opts = {}) {
  const base = opts.base ?? "/";
  const htmlFiles = await walkHtml(dir);

  /** @type {Set<string>} */
  const files = new Set();
  for (const abs of await walkOutputFiles(dir)) {
    const rel = relative(dir, abs).replace(/\\/g, "/");
    addDistOutputPaths(rel, files);
  }

  /** @type {{ path: string, title?: string, description?: string }[]} */
  const pages = [];
  /** @type {import("./findings.js").Finding[]} */
  const pageFindings = [];
  /** @type {{ from: string, href: string }[]} */
  const internalLinks = [];

  for (const abs of htmlFiles) {
    const rel = relative(dir, abs).replace(/\\/g, "/");
    const path = filePathToUrlPath(rel);

    const html = await readFile(abs, "utf8");
    const { document } = parseHTML(html);
    pageFindings.push(...auditPage(document, { path }));

    const title = (document.querySelector("title")?.textContent ?? "").trim();
    const description = (
      document.querySelector('meta[name="description"]')?.getAttribute("content") ?? ""
    ).trim();
    pages.push({ path, title, description });

    for (const a of document.querySelectorAll("a[href]")) {
      const raw = (a.getAttribute("href") ?? "").trim();
      if (!raw || isNonResolvableHref(raw)) continue;
      const resolved = resolveInternalHref(raw, path, base);
      if (resolved) internalLinks.push({ from: path, href: resolved });
    }
  }

  const siteFindings = auditSite({ pages, files, internalLinks });

  return {
    findings: [...pageFindings, ...siteFindings],
    pages: pages.map((p) => ({ path: p.path })),
  };
}

/**
 * @param {string} dir
 * @returns {Promise<string[]>}
 */
async function walkHtml(dir) {
  const all = await walkOutputFiles(dir);
  return all.filter((abs) => abs.replace(/\\/g, "/").endsWith(".html"));
}

/**
 * @param {string} dir
 * @returns {Promise<string[]>}
 */
async function walkOutputFiles(dir) {
  /** @type {string[]} */
  const out = [];
  async function walk(current) {
    const entries = await readdir(current, { withFileTypes: true });
    for (const ent of entries) {
      const p = join(current, ent.name);
      if (ent.isDirectory()) await walk(p);
      else out.push(p);
    }
  }
  await walk(dir);
  return out;
}

/**
 * @param {string} rel
 * @param {Set<string>} files
 */
function addDistOutputPaths(rel, files) {
  files.add(`/${rel}`);
  if (rel === "index.html") {
    files.add("/");
    return;
  }
  if (rel.endsWith("/index.html")) {
    const prefix = rel.slice(0, -"index.html".length);
    files.add(`/${prefix}`);
    const stem = prefix.replace(/\/$/, "");
    if (stem) files.add(`/${stem}.html`);
    return;
  }
  if (rel.endsWith(".html")) {
    const stem = rel.slice(0, -".html".length);
    files.add(`/${stem}/`);
  }
}

/**
 * @param {string} rel
 * @returns {string}
 */
export function filePathToUrlPath(rel) {
  if (rel === "index.html") return "/";
  if (rel.endsWith("/index.html")) {
    return `/${rel.slice(0, -"index.html".length)}`;
  }
  if (rel.endsWith(".html")) {
    const stem = rel.slice(0, -".html".length);
    return `/${stem}/`;
  }
  return `/${rel}/`;
}

/**
 * @param {string} href
 */
function isNonResolvableHref(href) {
  const lower = href.toLowerCase();
  return (
    lower.startsWith("http:") ||
    lower.startsWith("https:") ||
    lower.startsWith("mailto:") ||
    lower.startsWith("tel:") ||
    lower.startsWith("javascript:") ||
    href.startsWith("#")
  );
}

/**
 * @param {string} href
 * @param {string} fromPage
 * @param {string} base
 */
function resolveInternalHref(href, fromPage, base) {
  const stripped = href.split("#")[0].split("?")[0].trim();
  if (!stripped) return null;

  let pathname;
  if (stripped.startsWith("/")) {
    pathname = stripped;
    if (base !== "/") {
      const baseNorm = base.endsWith("/") ? base : `${base}/`;
      if (pathname === base || pathname === base.replace(/\/$/, "")) {
        pathname = "/";
      } else if (pathname.startsWith(baseNorm)) {
        pathname = `/${pathname.slice(baseNorm.length)}`;
      }
    }
  } else {
    const from = fromPage.endsWith("/") ? fromPage : `${fromPage}/`;
    pathname = new URL(stripped, `https://orso.local${from}`).pathname;
  }

  if (pathname.endsWith(".html/")) pathname = pathname.slice(0, -1);
  if (pathname === "/") return "/";
  const last = pathname.split("/").pop() ?? "";
  if (last.includes(".")) return pathname;
  if (!pathname.endsWith("/")) pathname += "/";
  return pathname;
}