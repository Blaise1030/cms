import type { AnyExtension, JSONContent } from "@tiptap/core";
import { Editor, mergeAttributes } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import Image from "@tiptap/extension-image";
import { Markdown, type MarkdownManager } from "@tiptap/markdown";

/**
 * The rich editor's node set: StarterKit without underline (no Markdown syntax) or the trailing
 * empty paragraph (it would add blank lines to the output), plus images,
 * read and written through the official `@tiptap/markdown` extension.
 */
export function markdownExtensions(resolveImage?: (src: string) => string): AnyExtension[] {
  return [
    StarterKit.configure({ underline: false, trailingNode: false, link: { openOnClick: false, autolink: false, linkOnPaste: false } }),
    resolveImage ? SafeImage.configure({ resolveSrc: resolveImage }) : SafeImage,
    Markdown,
  ];
}

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
  "codespan", "strong", "em", "del", "link", "image", "br", "hr", "escape",
]);

interface Token { type: string; raw?: string; text?: string; task?: boolean; tokens?: Token[]; items?: Token[] }

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
    if (token.type === "list_item" && token.task) return false;
    // MDX module syntax and expressions, and stray angle brackets, are not plain Markdown text.
    if (token.type === "paragraph" && /^\s*(import|export)\s/.test(token.raw ?? "")) return false;
    if (token.type === "text" && !token.tokens && /[{}<]/.test(token.raw ?? "")) return false;
    // Entities are decoded on parse and re-encoded differently on output.
    if (token.type === "text" && /&(#\d+|#x[\da-f]+|[a-z][a-z\d]*);/i.test(token.raw ?? "")) return false;
    if (!supportedTokens(token.tokens) || !supportedTokens(token.items)) return false;
  }
  return true;
}

const words = (source: string) => source.replace(/[^\p{L}\p{N}]/gu, "");
const same = (a: JSONContent, b: JSONContent) => JSON.stringify(a) === JSON.stringify(b);

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
