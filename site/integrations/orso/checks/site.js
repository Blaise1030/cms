import { createFinding } from "./findings.js";

/**
 * @param {{ pages: { path: string, title?: string, description?: string }[], files: Set<string>, internalLinks: { from: string, href: string }[] }} input
 * @returns {import("./findings.js").Finding[]}
 */
export function auditSite({ pages, files, internalLinks }) {
  /** @type {import("./findings.js").Finding[]} */
  const findings = [];

  findings.push(...duplicateFindings(pages, "title", "duplicate-title", "title"));
  findings.push(...duplicateFindings(pages, "description", "duplicate-description", "description"));

  if (!files.has("/robots.txt")) {
    findings.push(
      createFinding(
        "robots-missing",
        "warning",
        "/",
        "There's no robots.txt at the site root.",
      ),
    );
  }

  if (!files.has("/sitemap.xml")) {
    findings.push(
      createFinding(
        "sitemap-missing",
        "warning",
        "/",
        "There's no sitemap.xml at the site root.",
      ),
    );
  }

  findings.push(...brokenLinkFindings(internalLinks, files));

  return findings;
}

/**
 * @param {{ path: string, title?: string, description?: string }[]} pages
 * @param {"title" | "description"} field
 * @param {string} rule
 * @param {string} label
 */
function duplicateFindings(pages, field, rule, label) {
  /** @type {Map<string, string[]>} */
  const byValue = new Map();
  for (const page of pages) {
    const value = (page[field] ?? "").trim();
    if (!value) continue;
    const list = byValue.get(value) ?? [];
    list.push(page.path);
    byValue.set(value, list);
  }

  /** @type {import("./findings.js").Finding[]} */
  const findings = [];
  for (const paths of byValue.values()) {
    if (paths.length < 2) continue;
    for (const page of paths) {
      const others = paths.filter((p) => p !== page);
      const listed = others.slice(0, 5);
      findings.push(
        createFinding(
          rule,
          "warning",
          page,
          `Another page uses this ${label}.`,
          { detail: `Also used on ${listed.join(", ")}.` },
        ),
      );
    }
  }
  return findings;
}

/**
 * @param {{ from: string, href: string }[]} internalLinks
 * @param {Set<string>} files
 */
function brokenLinkFindings(internalLinks, files) {
  /** @type {Map<string, string[]>} */
  const brokenByFrom = new Map();

  for (const { from, href } of internalLinks) {
    const target = normalizePathForCheck(href);
    if (!target || existsAsPage(target, files)) continue;
    const list = brokenByFrom.get(from) ?? [];
    if (!list.includes(target)) list.push(target);
    brokenByFrom.set(from, list);
  }

  /** @type {import("./findings.js").Finding[]} */
  const findings = [];
  for (const [from, targets] of brokenByFrom) {
    const n = targets.length;
    const detail = targets.slice(0, 5).join(", ");
    findings.push(
      createFinding(
        "broken-link",
        "error",
        from,
        n === 1 ? "1 link is broken." : `${n} links are broken.`,
        { count: n, detail },
      ),
    );
  }
  return findings;
}

/**
 * @param {string} href
 */
function normalizePathForCheck(href) {
  const bare = href.split("#")[0].split("?")[0].trim();
  if (!bare || bare === "/") return "/";
  let path = bare.startsWith("/") ? bare : `/${bare}`;
  if (path.endsWith(".html/")) path = path.slice(0, -1);
  const last = path.split("/").pop() ?? "";
  if (last.includes(".")) return path;
  if (!path.endsWith("/")) path += "/";
  return path;
}

/**
 * @param {string} path
 * @param {Set<string>} files
 */
function existsAsPage(path, files) {
  for (const candidate of linkTargetCandidates(path)) {
    if (files.has(candidate)) return true;
  }
  return false;
}

/**
 * @param {string} path
 * @returns {string[]}
 */
function linkTargetCandidates(path) {
  /** @type {string[]} */
  const out = [path];
  if (path.endsWith("/")) {
    out.push(path.slice(0, -1));
    const stem = path.slice(0, -1);
    if (stem) out.push(`${stem}.html`);
  } else {
    out.push(`${path}/`);
    if (path.endsWith(".html")) {
      const stem = path.slice(0, -".html".length);
      out.push(`${stem}/`);
      if (stem) out.push(stem);
    } else if (!path.includes(".")) {
      out.push(`${path}.html`);
    }
  }
  return out;
}