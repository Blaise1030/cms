// Syntax highlighting via shiki, loaded lazily so it stays out of the first-load bundle.

const EXT_LANG: Record<string, string> = {
  astro: "astro",
  css: "css",
  scss: "scss",
  html: "html",
  js: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  jsx: "jsx",
  ts: "typescript",
  mts: "typescript",
  tsx: "tsx",
  json: "json",
  md: "markdown",
  mdx: "mdx",
  yml: "yaml",
  yaml: "yaml",
  toml: "toml",
  svg: "xml",
  xml: "xml",
  sh: "bash",
  vue: "vue",
  rs: "rust",
  py: "python",
};

/** Above this size we skip highlighting and show plain text. */
export const HIGHLIGHT_LIMIT = 200_000;

export function langForPath(path: string): string {
  const name = path.split("/").pop() ?? path;
  if (name === "Dockerfile") return "docker";
  const ext = name.includes(".") ? name.split(".").pop()!.toLowerCase() : "";
  return EXT_LANG[ext] ?? "text";
}

export function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

/** Returns trusted HTML (shiki escapes content; the fallback escapes it here). */
export async function highlightCode(code: string, path: string): Promise<string> {
  if (code.length > HIGHLIGHT_LIMIT) return `<pre class="shiki"><code>${escapeHtml(code)}</code></pre>`;
  try {
    const { codeToHtml } = await import("shiki");
    return await codeToHtml(code, { lang: langForPath(path), theme: "github-dark" });
  } catch {
    return `<pre class="shiki"><code>${escapeHtml(code)}</code></pre>`;
  }
}
