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
