import assert from "node:assert/strict";
import { test } from "node:test";

import { auditSite } from "../src/checks/site.js";

function site({ pages, files, internalLinks }) {
  return auditSite({
    pages,
    files: new Set(files),
    internalLinks,
  });
}

test("duplicate-title: same non-blank title on two pages", () => {
  const findings = site({
    pages: [
      { path: "/", title: "Home" },
      { path: "/about/", title: "Home" },
    ],
    files: ["/", "/about/"],
    internalLinks: [],
  });
  const dup = findings.filter((f) => f.rule === "duplicate-title");
  assert.equal(dup.length, 2);
  assert.ok(dup.every((f) => f.severity === "warning"));
  assert.ok(dup.some((f) => f.page === "/"));
  assert.ok(dup.some((f) => f.page === "/about/"));
  assert.match(dup[0].message, /Another page uses this title/);
});

test("duplicate-description: same non-blank description on two pages", () => {
  const findings = site({
    pages: [
      { path: "/", description: "Same" },
      { path: "/about/", description: "Same" },
    ],
    files: ["/", "/about/"],
    internalLinks: [],
  });
  const dup = findings.filter((f) => f.rule === "duplicate-description");
  assert.equal(dup.length, 2);
  assert.ok(dup.every((f) => f.severity === "warning"));
});

test("robots-missing and sitemap-missing when absent from files", () => {
  const findings = site({
    pages: [{ path: "/", title: "T" }],
    files: ["/"],
    internalLinks: [],
  });
  assert.ok(findings.some((f) => f.rule === "robots-missing" && f.page === "/"));
  assert.ok(findings.some((f) => f.rule === "sitemap-missing" && f.page === "/"));
});

test("robots and sitemap present: no site file findings", () => {
  const findings = site({
    pages: [{ path: "/", title: "T" }],
    files: ["/", "/robots.txt", "/sitemap.xml"],
    internalLinks: [],
  });
  assert.equal(findings.some((f) => f.rule === "robots-missing"), false);
  assert.equal(findings.some((f) => f.rule === "sitemap-missing"), false);
});

test("broken-link: existing asset path is not broken", () => {
  const findings = site({
    pages: [{ path: "/", title: "T" }],
    files: ["/", "/logo.png"],
    internalLinks: [{ from: "/", href: "/logo.png" }],
  });
  assert.equal(findings.some((f) => f.rule === "broken-link"), false);
});

test("broken-link: /about.html matches /about/ in files", () => {
  const findings = site({
    pages: [{ path: "/", title: "T" }],
    files: ["/", "/about/", "/about/index.html"],
    internalLinks: [{ from: "/", href: "/about.html" }],
  });
  assert.equal(findings.some((f) => f.rule === "broken-link"), false);
});

test("broken-link: internal href not in files", () => {
  const findings = site({
    pages: [{ path: "/", title: "T" }],
    files: ["/"],
    internalLinks: [{ from: "/", href: "/gone/" }],
  });
  const broken = findings.find((f) => f.rule === "broken-link");
  assert.ok(broken);
  assert.equal(broken.severity, "error");
  assert.equal(broken.page, "/");
  assert.match(broken.message, /broken/i);
});