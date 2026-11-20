// Enforces the design-token rule in AGENTS.md: colours come from the tokens in the @theme block
// of src/styles/global.css, never from literals, Tailwind's palette or arbitrary values.
import assert from "node:assert/strict";
import { test } from "node:test";
import { readdirSync, readFileSync } from "node:fs";
import { join, relative, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const TOKENS_FILE = "src/styles/global.css";
const EXTENSIONS = /\.(astro|ts|vue|mdx|css)$/;

const PALETTE = "slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose";
const COLOR_UTILITIES = "bg|text|border(?:-[xytrblse])?|ring(?:-offset)?|outline|divide|fill|stroke|from|via|to|placeholder|caret|accent|decoration|shadow|inset-shadow|drop-shadow";
const RULES = [
  { name: "raw hex colour", pattern: /(?<![&\w])#(?:[0-9a-f]{8}|[0-9a-f]{6}|[0-9a-f]{3,4})\b/gi },
  { name: "raw colour function", pattern: /\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch)\(/gi },
  {
    name: "Tailwind palette colour",
    pattern: new RegExp(`(?<![\\w-])(?:[\\w-]+:)*(?:${COLOR_UTILITIES})-(?:(?:${PALETTE})-\\d{2,3}|white|black)(?:\\/\\d+)?(?![\\w-])`, "g"),
  },
  {
    name: "arbitrary colour value",
    pattern: new RegExp(`(?<![\\w-])(?:[\\w-]+:)*(?:${COLOR_UTILITIES})-\\[(?:#|(?:rgba?|hsla?|oklch|oklab|color)\\b|white|black)`, "gi"),
  },
  { name: "dark: colour variant (redefine the token instead)", pattern: new RegExp(`(?<![\\w-])dark:(?:${COLOR_UTILITIES})-|\\bprose-invert\\b`, "g") },
  { name: "inline style colour", pattern: /style=(?:"|\{\{?|`)[^"}`]*\b(?:color|background|fill|stroke|border-color)\s*:\s*(?![\s]|var\()/gi },
];

/** Blank out what may legitimately look like a colour: link targets (`#faq`) and comments. */
function scrub(source) {
  return source
    .replace(/href(?:=|:\s*)(?:"[^"]*"|'[^']*'|\{[^}]*\}|`[^`]*`)/g, (m) => " ".repeat(m.length))
    .replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, " "))
    .replace(/<!--[\s\S]*?-->/g, (m) => m.replace(/[^\n]/g, " "));
}

/** Every rule violation in `source`; in the tokens file, custom-property definitions are allowed. */
export function findViolations(source, file = "") {
  const tokensFile = file === TOKENS_FILE;
  const found = [];
  scrub(source).split("\n").forEach((line, index) => {
    if (tokensFile && /^\s*--[\w-]+(?:-\*)?\s*:/.test(line)) return;
    for (const rule of RULES) {
      for (const match of line.matchAll(rule.pattern)) found.push({ file, line: index + 1, rule: rule.name, text: match[0] });
    }
  });
  return found;
}

function* walk(dir) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) yield* walk(path);
    else if (EXTENSIONS.test(entry.name)) yield path;
  }
}

test("the scanner flags hard-coded colours and allows tokens", () => {
  const bad = [
    `<p class="text-neutral-600">`,
    `<div class="bg-white dark:bg-black">`,
    `<p class="hover:border-blue-500/50">`,
    `<p class="text-[#333]">`,
    `<p class="bg-[rgb(0,0,0)]">`,
    `<p class="dark:text-muted">`,
    `<div class="prose dark:prose-invert">`,
    `<p style="color: red">`,
    `const brand = "#3b5a72";`,
    `.x { color: oklch(0.5 0.1 200); }`,
  ];
  for (const sample of bad) assert.ok(findViolations(sample).length > 0, `expected a violation in: ${sample}`);
  const good = [
    `<p class="bg-surface text-muted border-line font-display duration-gentle">`,
    `<p class="bg-surface/90 fill-ridge/40 aria-[current=page]:text-ink">`,
    `<a href="#faq">FAQ</a> <a href={\`#\${slug}\`}>x</a> { label: "Add", href: "#add" }`,
    `<p style="color: var(--color-ink)">`,
    `.x { color: var(--color-ink); background: color-mix(in oklab, var(--color-ink) 5%, transparent); }`,
    `&#8212;`,
  ];
  for (const sample of good) assert.deepEqual(findViolations(sample), [], `unexpected violation in: ${sample}`);
  assert.deepEqual(findViolations("  --color-surface: #f6f4ef;", TOKENS_FILE), []);
  assert.ok(findViolations("  color: #f6f4ef;", TOKENS_FILE).length > 0);
});

test("src uses design tokens, not hard-coded colours (see AGENTS.md)", () => {
  const violations = [...walk(join(root, "src"))].flatMap((path) => {
    const file = relative(root, path).split("\\").join("/");
    return findViolations(readFileSync(path, "utf8"), file);
  });
  assert.deepEqual(
    violations.map((v) => `${v.file}:${v.line} ${v.rule}: ${v.text}`),
    [],
    "Hard-coded colours found. AGENTS.md: use the tokens in the @theme block of src/styles/global.css " +
      "(bg-surface, text-muted, border-line, …) or var(--…); add a token there and document it in DESIGN.md when one is missing.",
  );
});
