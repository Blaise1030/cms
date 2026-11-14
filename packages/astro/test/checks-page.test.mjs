import assert from "node:assert/strict";
import { test } from "node:test";
import { parseHTML } from "linkedom";

import { auditPage } from "../src/checks/page.js";

function audit(html, path = "/") {
  const { document } = parseHTML(html);
  return auditPage(document, { path });
}

function expectRule(html, rule, severity) {
  const findings = audit(html);
  const match = findings.find((f) => f.rule === rule);
  assert.ok(match, `expected finding rule=${rule}`);
  assert.equal(match.severity, severity);
}

function expectNoRule(html, rule) {
  const findings = audit(html);
  assert.equal(
    findings.some((f) => f.rule === rule),
    false,
    `did not expect rule=${rule}`,
  );
}

const wrap = (body, head = "") =>
  `<!DOCTYPE html><html lang="en"><head>${head}</head><body>${body}</body></html>`;

test("title-missing: no title", () => {
  expectRule(wrap("<p>x</p>", ""), "title-missing", "error");
});

test("title-length: 61 characters", () => {
  const title = "a".repeat(61);
  expectRule(wrap("<p>x</p>", `<title>${title}</title>`), "title-length", "warning");
});

test("description-missing: no meta description", () => {
  expectRule(wrap("<p>x</p>", "<title>Hi</title>"), "description-missing", "warning");
});

test("description-length: 161 characters", () => {
  const desc = "d".repeat(161);
  expectRule(
    wrap(
      "<p>x</p>",
      `<title>Hi</title><meta name="description" content="${desc}">`,
    ),
    "description-length",
    "warning",
  );
});

test("heading: no h1", () => {
  expectRule(wrap("<p>x</p>", "<title>Hi</title>"), "heading", "error");
});

test("heading: two h1 elements", () => {
  expectRule(
    wrap("<h1>a</h1><h1>b</h1>", "<title>Hi</title>"),
    "heading",
    "warning",
  );
});

test("image-alt: img without alt", () => {
  expectRule(wrap('<img src="x.png">', "<title>Hi</title>"), "image-alt", "warning");
});

test("image-alt: empty alt is allowed", () => {
  expectNoRule(wrap('<img src="x.png" alt="">', "<title>Hi</title>"), "image-alt");
});

test("html-lang: missing lang", () => {
  const html = `<!DOCTYPE html><html><head><title>Hi</title></head><body><h1>x</h1></body></html>`;
  expectRule(html, "html-lang", "warning");
});

test("social-preview: missing og tags", () => {
  expectRule(wrap("<h1>x</h1>", "<title>Hi</title>"), "social-preview", "warning");
});

test("noindex: robots noindex", () => {
  expectRule(
    wrap("<h1>x</h1>", '<title>Hi</title><meta name="robots" content="noindex">'),
    "noindex",
    "warning",
  );
});

test("canonical-missing: no canonical link", () => {
  expectRule(wrap("<h1>x</h1>", "<title>Hi</title>"), "canonical-missing", "warning");
});

test("json-ld-missing: no JSON-LD", () => {
  expectRule(wrap("<h1>x</h1>", "<title>Hi</title>"), "json-ld-missing", "warning");
});

test("json-ld-invalid: broken JSON-LD", () => {
  expectRule(
    wrap(
      "<h1>x</h1>",
      '<title>Hi</title><script type="application/ld+json">{ bad</script>',
    ),
    "json-ld-invalid",
    "error",
  );
});

test("json-ld-type: only Thing type", () => {
  expectRule(
    wrap(
      "<h1>x</h1>",
      `<title>Hi</title><script type="application/ld+json">{"@context":"https://schema.org","@type":"Thing"}</script>`,
    ),
    "json-ld-type",
    "warning",
  );
});

test("faq-structure: details without FAQPage JSON-LD warns", () => {
  const html = wrap(
    "<details><summary>Q?</summary><p>A</p></details>",
    "<title>Hi</title>",
  );
  const findings = audit(html);
  const faq = findings.find((f) => f.rule === "faq-structure");
  assert.ok(faq, "expected faq-structure finding");
  assert.equal(faq.severity, "warning");
});

test("img-dimensions: missing width and height", () => {
  expectRule(
    wrap('<img src="x.png" alt="x">', "<title>Hi</title>"),
    "img-dimensions",
    "warning",
  );
});

test("sync-script: blocking script in head", () => {
  expectRule(
    wrap("<h1>x</h1>", '<title>Hi</title><script src="/app.js"></script>'),
    "sync-script",
    "warning",
  );
});

test("huge-inline: script over 50 KiB", () => {
  const big = "x".repeat(50 * 1024 + 1);
  expectRule(
    wrap("<h1>x</h1>", `<title>Hi</title><script>${big}</script>`),
    "huge-inline",
    "warning",
  );
});