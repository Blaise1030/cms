import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import { auditDist } from "../src/checks/audit.js";
import { exitCode, formatReport } from "../src/checks/report.js";

const goodPage = (body, headExtra = "") => `<!DOCTYPE html>
<html lang="en"><head>
<title>Home</title>
<meta name="description" content="Welcome home.">
${headExtra}
</head><body><h1>Home</h1>${body}</body></html>`;

const pricingPage = (body = "") => `<!DOCTYPE html>
<html lang="en"><head><title>Pricing</title></head>
<body><h1>Pricing</h1>${body}</body></html>`;

async function fixtureDir(setup) {
  const dir = await mkdtemp(join(tmpdir(), "orso-audit-"));
  await setup(dir);
  return dir;
}

test("auditDist: description-missing and broken-link from fixture dist", async () => {
  const dir = await fixtureDir(async (root) => {
    await writeFile(
      join(root, "index.html"),
      goodPage('<a href="/gone/">gone</a>'),
    );
    await mkdir(join(root, "pricing"), { recursive: true });
    await writeFile(join(root, "pricing", "index.html"), pricingPage());
  });

  try {
    const { findings } = await auditDist(dir);
    assert.ok(findings.some((f) => f.rule === "description-missing" && f.page === "/pricing/"));
    assert.ok(findings.some((f) => f.rule === "broken-link"));
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("exitCode: 1 when any error, 0 when only warnings", async () => {
  assert.equal(
    exitCode([{ severity: "warning", rule: "x", page: "/", message: "m" }]),
    0,
  );
  assert.equal(
    exitCode([
      { severity: "warning", rule: "x", page: "/", message: "m" },
      { severity: "error", rule: "broken-link", page: "/", message: "m" },
    ]),
    1,
  );
});

test("auditDist with base: prefixed href to existing page is not broken-link", async () => {
  const dir = await fixtureDir(async (root) => {
    await writeFile(join(root, "index.html"), goodPage('<a href="/preview/x/pricing/">pricing</a>'));
    await mkdir(join(root, "pricing"), { recursive: true });
    await writeFile(join(root, "pricing", "index.html"), pricingPage());
  });

  try {
    const { findings } = await auditDist(dir, { base: "/preview/x/" });
    assert.equal(
      findings.some((f) => f.rule === "broken-link"),
      false,
      "expected no broken-link for base-prefixed internal href",
    );
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("formatReport: returns non-empty string for findings", () => {
  const text = formatReport([
    { rule: "description-missing", severity: "warning", page: "/pricing/", message: "This page has no description." },
  ]);
  assert.ok(text.length > 0);
  assert.match(text, /description-missing|no description/i);
});