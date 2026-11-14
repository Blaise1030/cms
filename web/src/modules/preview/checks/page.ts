import { createFinding, type Finding } from "./findings";

const TITLE_MAX = 60;
const DESCRIPTION_MAX = 160;
const HUGE_INLINE_BYTES = 50 * 1024;

const JSON_LD_TYPES = new Set([
  "Organization",
  "WebSite",
  "WebPage",
  "Article",
  "BlogPosting",
  "FAQPage",
  "HowTo",
  "BreadcrumbList",
]);

export function auditPage(doc: ParentNode, { path }: { path: string }): Finding[] {
  const findings: Finding[] = [];

  const titleEl = doc.querySelector("title");
  const title = (titleEl?.textContent ?? "").trim();
  if (!title) {
    findings.push(
      createFinding("title-missing", "error", path, "This page has no title."),
    );
  } else if (title.length > TITLE_MAX) {
    findings.push(
      createFinding(
        "title-length",
        "warning",
        path,
        `The title is ${title.length} characters; keep it under ${TITLE_MAX}.`,
      ),
    );
  }

  const descEl = doc.querySelector('meta[name="description"]');
  const description = (descEl?.getAttribute("content") ?? "").trim();
  if (!description) {
    findings.push(
      createFinding(
        "description-missing",
        "warning",
        path,
        "This page has no description.",
      ),
    );
  } else if (description.length > DESCRIPTION_MAX) {
    findings.push(
      createFinding(
        "description-length",
        "warning",
        path,
        `The description is ${description.length} characters; keep it under ${DESCRIPTION_MAX}.`,
      ),
    );
  }

  const h1s = doc.querySelectorAll("h1");
  if (h1s.length === 0) {
    findings.push(
      createFinding(
        "heading",
        "error",
        path,
        "This page has no main heading.",
      ),
    );
  } else if (h1s.length > 1) {
    findings.push(
      createFinding(
        "heading",
        "warning",
        path,
        `This page has ${h1s.length} main headings; keep one.`,
        { count: h1s.length },
      ),
    );
  }

  const imgsNoAlt = [...doc.querySelectorAll("img")].filter(
    (img) => !img.hasAttribute("alt"),
  );
  if (imgsNoAlt.length > 0) {
    const n = imgsNoAlt.length;
    findings.push(
      createFinding(
        "image-alt",
        "warning",
        path,
        n === 1 ? "1 image has no alt text." : `${n} images have no alt text.`,
        { count: n },
      ),
    );
  }

  const htmlEl = doc.querySelector("html");
  const lang = (htmlEl?.getAttribute("lang") ?? "").trim();
  if (!lang) {
    findings.push(
      createFinding(
        "html-lang",
        "warning",
        path,
        "The page doesn't declare a language.",
      ),
    );
  }

  const ogMissing = ["og:title", "og:description", "og:image"].filter(
    (prop) => !doc.querySelector(`meta[property="${prop}"]`),
  );
  if (ogMissing.length > 0) {
    findings.push(
      createFinding(
        "social-preview",
        "warning",
        path,
        "This page has no social preview tags.",
        { detail: `Missing ${ogMissing.join(", ")}.` },
      ),
    );
  }

  const robots = (
    doc.querySelector('meta[name="robots"]')?.getAttribute("content") ?? ""
  ).toLowerCase();
  if (robots.includes("noindex")) {
    findings.push(
      createFinding(
        "noindex",
        "warning",
        path,
        "Search engines are told not to index this page.",
      ),
    );
  }

  const canonical = doc.querySelector('link[rel="canonical"]');
  if (!canonical?.getAttribute("href")?.trim()) {
    findings.push(
      createFinding(
        "canonical-missing",
        "warning",
        path,
        "This page has no canonical URL.",
      ),
    );
  }

  const ldScripts = [...doc.querySelectorAll('script[type="application/ld+json"]')];
  const parsedBlocks = [];
  let invalidLd = false;

  if (ldScripts.length === 0) {
    findings.push(
      createFinding(
        "json-ld-missing",
        "warning",
        path,
        "This page has no structured data (JSON-LD).",
      ),
    );
  } else {
    for (const script of ldScripts) {
      const raw = (script.textContent ?? "").trim();
      if (!raw) continue;
      try {
        parsedBlocks.push(JSON.parse(raw));
      } catch {
        invalidLd = true;
      }
    }
    if (invalidLd) {
      findings.push(
        createFinding(
          "json-ld-invalid",
          "error",
          path,
          "Structured data on this page is not valid JSON.",
        ),
      );
    } else if (parsedBlocks.length > 0 && !hasUsefulJsonLdType(parsedBlocks)) {
      findings.push(
        createFinding(
          "json-ld-type",
          "warning",
          path,
          "Structured data does not include a useful schema type.",
        ),
      );
    }
  }

  const hasFaqPattern = doc.querySelector("details") !== null;
  if (hasFaqPattern && !jsonLdHasType(parsedBlocks, "FAQPage")) {
    findings.push(
      createFinding(
        "faq-structure",
        "warning",
        path,
        "This page looks like FAQ content but has no FAQPage structured data.",
      ),
    );
  }

  const imgsNoDims = [...doc.querySelectorAll("img")].filter((img) => {
    const w = img.getAttribute("width");
    const h = img.getAttribute("height");
    return !(w?.trim() && h?.trim());
  });
  if (imgsNoDims.length > 0) {
    const n = imgsNoDims.length;
    findings.push(
      createFinding(
        "img-dimensions",
        "warning",
        path,
        n === 1
          ? "1 image is missing width and height."
          : `${n} images are missing width and height.`,
        { count: n },
      ),
    );
  }

  const headScripts = doc.querySelectorAll("head script[src]");
  const blocking = [...headScripts].filter((script) => {
    if (script.getAttribute("defer") != null) return false;
    if (script.getAttribute("async") != null) return false;
    const type = (script.getAttribute("type") ?? "").toLowerCase();
    if (type === "module") return false;
    return true;
  });
  if (blocking.length > 0) {
    findings.push(
      createFinding(
        "sync-script",
        "warning",
        path,
        "A render-blocking script in the page head may slow loading.",
        { count: blocking.length },
      ),
    );
  }

  const huge = [];
  for (const el of doc.querySelectorAll("script:not([src]), style")) {
    const bytes = new TextEncoder().encode(el.textContent ?? "").length;
    if (bytes > HUGE_INLINE_BYTES) {
      huge.push(el);
    }
  }
  if (huge.length > 0) {
    findings.push(
      createFinding(
        "huge-inline",
        "warning",
        path,
        "Large inline script or style may slow the page.",
        { count: huge.length },
      ),
    );
  }

  return findings;
}

/**
 * @param {unknown[]} blocks
 */
function hasUsefulJsonLdType(blocks) {
  for (const block of blocks) {
    if (jsonLdHasAllowlistedType(block)) return true;
  }
  return false;
}

/**
 * @param {unknown[]} blocks
 * @param {string} type
 */
function jsonLdHasType(blocks, type) {
  for (const block of blocks) {
    if (nodeHasType(block, type)) return true;
  }
  return false;
}

/**
 * @param {unknown} node
 */
function jsonLdHasAllowlistedType(node) {
  return collectTypes(node).some((t) => JSON_LD_TYPES.has(t));
}

/**
 * @param {unknown} node
 * @param {string} type
 */
function nodeHasType(node, type) {
  return collectTypes(node).includes(type);
}

/**
 * @param {unknown} node
 * @returns {string[]}
 */
function collectTypes(node) {
  /** @type {string[]} */
  const types = [];
  walkJsonLd(node, types);
  return types;
}

/**
 * @param {unknown} node
 * @param {string[]} types
 */
function walkJsonLd(node, types) {
  if (node == null || typeof node !== "object") return;

  if (Array.isArray(node)) {
    for (const item of node) walkJsonLd(item, types);
    return;
  }

  /** @type {Record<string, unknown>} */
  const obj = /** @type {Record<string, unknown>} */ (node);

  const atType = obj["@type"];
  if (typeof atType === "string") {
    types.push(atType);
  } else if (Array.isArray(atType)) {
    for (const t of atType) {
      if (typeof t === "string") types.push(t);
    }
  }

  if (Array.isArray(obj["@graph"])) {
    walkJsonLd(obj["@graph"], types);
  }
}