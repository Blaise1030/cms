import type { AnyExtension, JSONContent } from "@tiptap/core";
import { Editor, mergeAttributes } from "@tiptap/core";
import StarterKit, { type StarterKitOptions } from "@tiptap/starter-kit";
import Image from "@tiptap/extension-image";
import { OrderedList, TaskItem, TaskList } from "@tiptap/extension-list";
import { renderTableToMarkdown, Table, TableKit } from "@tiptap/extension-table";
import { Markdown, MarkdownManager } from "@tiptap/markdown";

/**
 * Tiptap writes every `&` in text as `&amp;`, so "Voice & Tone" saved as "Voice &amp; Tone" and
 * failed the round-trip check. A bare `&` is literal in CommonMark; only one that would read as an
 * entity needs escaping. Code text comes back unchanged and is left alone.
 */
type EncodeText = (text: string, node: unknown, parent: unknown) => string;
const encodeProto = MarkdownManager.prototype as unknown as { encodeTextForMarkdown: EncodeText };
const encodeText = encodeProto.encodeTextForMarkdown;
encodeProto.encodeTextForMarkdown = function (this: MarkdownManager, text, node, parent) {
  const out = encodeText.call(this, text, node, parent);
  return out === text ? out : out.replace(/&amp;(?!#\d+;|#x[\da-f]+;|[a-z][a-z\d]*;)/gi, "&");
};

/**
 * Tiptap writes every link as `[text](href)`, so a bare address that Markdown links on its own
 * (`privacy@example.com`, `https://x.test`, `www.x.test`, `<ftp://x.test>`) gained text on save and
 * failed the round-trip check. When a link's text is its own address, write it as an autolink:
 * bare where GFM links it, in angle brackets otherwise. Any other link keeps `[text](href)`.
 */
type MarkSyntax = (markType: string, mark: { attrs?: Record<string, unknown> }, openingMode?: string, markText?: string) => string;
const markProto = MarkdownManager.prototype as unknown as { getMarkOpening: MarkSyntax; getMarkClosing: MarkSyntax };
const EMAIL = /^[\w.+-]+@[a-z\d-]+(\.[a-z\d-]+)+$/i;
function autolink(markType: string, mark: { attrs?: Record<string, unknown> }, openingMode?: string, text?: string): "bare" | "angle" | null {
  if (markType !== "link" || openingMode === "html" || !text || /[\s<>\\`*_[\]]/.test(text)) return null;
  const href = mark.attrs?.href;
  if (typeof href !== "string" || mark.attrs?.title) return null;
  if (href === `mailto:${text}` && EMAIL.test(text)) return "bare";
  if (href === `http://${text}` && /^www\./i.test(text)) return "bare";
  if (href !== text) return null;
  if (/^https?:\/\/[^\s]+[\w/]$/i.test(text)) return "bare";
  return /^[a-z][a-z\d+.-]{1,31}:[^\s<>]*$/i.test(text) ? "angle" : null;
}
const markOpening = markProto.getMarkOpening;
const markClosing = markProto.getMarkClosing;
markProto.getMarkOpening = function (this: MarkdownManager, markType, mark, openingMode, markText) {
  const kind = autolink(markType, mark, openingMode, markText);
  return kind ? (kind === "angle" ? "<" : "") : markOpening.call(this, markType, mark, openingMode, markText);
};
markProto.getMarkClosing = function (this: MarkdownManager, markType, mark, openingMode, markText) {
  const kind = autolink(markType, mark, openingMode, markText);
  return kind ? (kind === "angle" ? ">" : "") : markClosing.call(this, markType, mark, openingMode, markText);
};

/**
 * StarterKit for the rich editor: without underline (no Markdown syntax), the trailing empty
 * paragraph (it would add blank lines to the output) or its letter-reading ordered list.
 * Passed to Nuxt UI's `UEditor` as `starter-kit`.
 */
export const STARTER_KIT: Partial<StarterKitOptions> = {
  underline: false,
  trailingNode: false,
  orderedList: false,
  link: { openOnClick: false, autolink: false, linkOnPaste: false },
};

/** The extensions the rich editor adds on top of what `UEditor` builds in: lists, tables and images. */
export function bodyExtensions(resolveImage?: (src: string) => string): AnyExtension[] {
  return [
    NumericOrderedList,
    TaskList,
    TaskItem.configure({ nested: true }),
    TableKit.configure({ table: false }),
    TightTable.configure({ resizable: false }),
    resolveImage ? SafeImage.configure({ resolveSrc: resolveImage }) : SafeImage,
  ];
}

/** One of StarterKit's own extensions, built the way StarterKit builds it (the packages are not direct dependencies). */
function kitExtension(name: "code" | "horizontalRule"): AnyExtension {
  const addExtensions = StarterKit.config.addExtensions as unknown as (this: { options: object }) => AnyExtension[];
  return addExtensions.call({ options: {} }).find(extension => extension.name === name)!;
}

/**
 * The rich editor's exact schema, for use outside `UEditor` (the round-trip check, tests).
 * `UEditor` (content-type markdown) adds `Markdown` with GFM, StarterKit with its own code mark
 * (which may share text with other marks) and horizontal rule appended after it, then
 * `bodyExtensions`; this mirrors that order so marks serialize the same way.
 */
export function markdownExtensions(resolveImage?: (src: string) => string): AnyExtension[] {
  return [
    Markdown.configure({ markedOptions: { gfm: true } }),
    StarterKit.configure({ ...STARTER_KIT, code: false, horizontalRule: false }),
    kitExtension("code").extend({ excludes: "code" }),
    kitExtension("horizontalRule"),
    ...bodyExtensions(resolveImage),
  ];
}

/**
 * Tiptap's table serializer wraps every table in an extra newline, so saving added a blank line
 * at the top of a body that starts with a table and a third newline around any other. The block
 * separator already spaces tables like other blocks; trim the wrapper.
 */
const TightTable = Table.extend({
  renderMarkdown: (node, h) => renderTableToMarkdown(node, h).replace(/^\n+|\n+$/g, ""),
});

/**
 * Ordered lists that start only at a numeric marker. Tiptap also reads letter markers, so a
 * paragraph like "No. Meta restricts…" became a list and serialized as "1. Meta restricts…".
 */
const NumericOrderedList = OrderedList.extend({
  markdownTokenizer: {
    ...OrderedList.config.markdownTokenizer!,
    tokenize(src, tokens, lexer) {
      if (!/^\s*\d+[.)]\s/.test(src)) return undefined;
      return OrderedList.config.markdownTokenizer!.tokenize.call(this, src, tokens, lexer);
    },
  },
});

/**
 * The project path an entry-relative image link points at (`../../../assets/x.png` from
 * `src/content/blog/en/a.md` → `src/assets/x.png`), or null for absolute, scheme, or escaping links.
 */
export function entryAssetPath(entryPath: string, src: string): string | null {
  if (!src || /^[a-z][a-z\d+.-]*:/i.test(src) || src.startsWith("/") || src.startsWith("#")) return null;
  const parts = entryPath.split("/").slice(0, -1);
  let decoded: string;
  // A malformed escape (`100%.png`) is left alone: no display rewrite.
  try { decoded = decodeURI(src.split(/[?#]/)[0]!); } catch { return null; }
  for (const part of decoded.split("/")) {
    if (part === "..") { if (!parts.length) return null; parts.pop(); }
    else if (part && part !== ".") parts.push(part);
  }
  return parts.length ? parts.join("/") : null;
}

/**
 * Images whose alt text keeps its brackets escaped. The parser unescapes only `\[` and `\]` in
 * alt text, so escaping exactly those is its inverse and an unbalanced bracket cannot end the
 * image early.
 */
const SafeImage = Image.extend<{ resolveSrc: ((src: string) => string) | null } & Record<string, unknown>>({
  addOptions() { return { ...this.parent?.(), resolveSrc: null }; },
  // Copied or re-parsed HTML carries the link as written in `data-src`; the shown `src` may be a
  // display URL that must never reach the Markdown.
  addAttributes() {
    return {
      ...this.parent?.(),
      src: { default: null, parseHTML: (el: HTMLElement) => el.getAttribute("data-src") ?? el.getAttribute("src") },
    };
  },
  // Display only: the node keeps the link as written, so Markdown output stays relative.
  renderHTML({ HTMLAttributes }) {
    const resolve = this.options.resolveSrc as ((src: string) => string) | null;
    const written = HTMLAttributes.src;
    const src = resolve && typeof written === "string" ? resolve(written) : written;
    const original = src !== written ? { "data-src": written } : {};
    return ["img", mergeAttributes(this.options.HTMLAttributes as Record<string, unknown>, HTMLAttributes, { src, ...original })];
  },
  renderMarkdown: (node: { attrs?: Record<string, unknown> }) => {
    const src = String(node.attrs?.src ?? "");
    const alt = String(node.attrs?.alt ?? "").replace(/\s*\n\s*/g, " ").replace(/[[\]]/g, c => `\\${c}`);
    const title = node.attrs?.title ? String(node.attrs.title) : "";
    return title ? `![${alt}](${src} "${title}")` : `![${alt}](${src})`;
  },
});

/** marked token types the configured extensions parse and serialize without loss. */
const SUPPORTED_TOKENS = new Set([
  "space", "paragraph", "text", "heading", "list", "list_item", "blockquote", "code",
  "codespan", "strong", "em", "del", "link", "image", "br", "hr", "escape", "table",
  "checkbox", "taskList", "taskItem",
]);

type Cell = { tokens: Token[] };
interface Token { type: string; raw?: string; text?: string; task?: boolean; tokens?: Token[]; items?: Token[]; header?: Cell[]; rows?: Cell[][] }

let manager: MarkdownManager | null = null;
/** A detached editor gives the same flattened extension set and Markdown manager the rich editor uses. */
function markdownManager(): MarkdownManager {
  if (!manager) {
    const editor = new Editor({ extensions: markdownExtensions() });
    manager = editor.markdown!;
  }
  return manager;
}

/** Only checks Markdown structure: any token outside the supported set, or MDX-looking text, fails. */
function supportedTokens(tokens: Token[] | undefined): boolean {
  for (const token of tokens ?? []) {
    if (!SUPPORTED_TOKENS.has(token.type)) return false;
    // MDX module syntax and expressions, and stray angle brackets, are not plain Markdown text.
    if (token.type === "paragraph" && /^\s*(import|export)\s/.test(token.raw ?? "")) return false;
    if (token.type === "text" && !token.tokens && /[{}<]/.test(token.raw ?? "")) return false;
    // Entities are decoded on parse and re-encoded differently on output.
    if (token.type === "text" && /&(#\d+|#x[\da-f]+|[a-z][a-z\d]*);/i.test(token.raw ?? "")) return false;
    if (!supportedTokens(token.tokens) || !supportedTokens(token.items)) return false;
    const cells = [...(token.header ?? []), ...(token.rows ?? []).flat()];
    if (!cells.every(cell => supportedTokens(cell.tokens))) return false;
  }
  return true;
}

const words = (source: string) => source.replace(/[^\p{L}\p{N}]/gu, "");

/**
 * Compare parsed documents ignoring insignificant line-wrap whitespace inside text nodes.
 * Code blocks keep their text byte-for-byte.
 */
function canonical(node: JSONContent): JSONContent {
  if (node.type === "codeBlock") return node;
  return {
    ...node,
    ...(typeof node.text === "string" ? { text: node.text.replace(/\s+/g, " ") } : {}),
    ...(node.content ? { content: node.content.map(canonical) } : {}),
  };
}
const same = (a: JSONContent, b: JSONContent) =>
  JSON.stringify(canonical(a)) === JSON.stringify(canonical(b));

/**
 * Whether `source` can be edited in rich mode without losing anything. Conservative: every token must
 * be in the supported set, the parsed document must survive serialize/parse unchanged, and the
 * serialized Markdown must keep every letter and digit of the source in order.
 */
export function canRoundTrip(source: string): boolean {
  try {
    const md = markdownManager();
    const tokens = md.instance.lexer(source) as unknown as Token[] & { links?: Record<string, unknown> };
    if (tokens.links && Object.keys(tokens.links).length) return false;
    if (!supportedTokens(tokens)) return false;
    const parsed = md.parse(source);
    const serialized = md.serialize(parsed);
    if (!same(parsed, md.parse(serialized))) return false;
    return words(serialized) === words(source);
  } catch {
    return false;
  }
}
