/**
 * Code first (a fenced block, or an inline span with its matching run of backticks), so a
 * comment inside code is part of the code; otherwise an HTML comment.
 */
const CODE_OR_COMMENT =
  /(^ {0,3}(`{3,}|~{3,})[^\n]*\n[\s\S]*?(?:^ {0,3}\2[`~]*[ \t]*$|(?![\s\S])))|(`+)(?!`)[\s\S]*?[^`]\3(?!`)|<!--[\s\S]*?-->/gm;

/**
 * Drops HTML comments (`<!-- … -->`) the way GitHub hides them, e.g. a bot's marker comment,
 * leaving fenced and inline code untouched.
 */
export function stripHtmlComments(markdown: string): string {
  if (!markdown.includes("<!--")) return markdown;
  return markdown.replace(CODE_OR_COMMENT, (match, fence: string | undefined, _f, tick: string | undefined) =>
    fence !== undefined || tick !== undefined ? match : "",
  );
}

export type MarkdownBlock = { kind: "text"; text: string } | { kind: "images"; images: { alt: string; url: string }[] };

/** A line holding only images, e.g. `![a](x.png) ![b](y.png)`. */
const IMAGE = /!\[([^\]]*)\]\(\s*<?([^)\s>]+)>?(?:\s+"[^"]*")?\s*\)/g;
const IMAGES_ONLY = /^\s*(?:!\[[^\]]*\]\([^)]*\)\s*)+$/;
const FENCE = /^ {0,3}(`{3,}|~{3,})/;

/**
 * Splits markdown so runs of image-only lines (blank lines between them allowed) can show as a
 * grid. Everything else, code blocks included, stays text in its original order.
 */
export function splitImageRuns(markdown: string): MarkdownBlock[] {
  const blocks: MarkdownBlock[] = [];
  let text: string[] = [];
  let images: { alt: string; url: string }[] = [];
  let fence: string | null = null;
  const flushText = () => {
    if (text.join("").trim()) blocks.push({ kind: "text", text: text.join("\n") });
    text = [];
  };
  const flushImages = () => {
    if (images.length) blocks.push({ kind: "images", images });
    images = [];
  };
  for (const line of markdown.split("\n")) {
    const open = FENCE.exec(line)?.[1];
    if (fence) {
      if (open && open[0] === fence[0] && open.length >= fence.length) fence = null;
      text.push(line);
      continue;
    }
    if (open) {
      flushImages();
      fence = open;
      text.push(line);
      continue;
    }
    if (IMAGES_ONLY.test(line)) {
      flushText();
      for (const m of line.matchAll(IMAGE)) images.push({ alt: m[1]!, url: m[2]! });
      continue;
    }
    // A blank line between images keeps the run going.
    if (!line.trim() && images.length) continue;
    flushImages();
    text.push(line);
  }
  flushImages();
  flushText();
  return blocks;
}
