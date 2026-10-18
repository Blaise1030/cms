// Single source of truth for "what is the app shell". Used by the service-worker manifest
// transform (pwa.config.ts) and the post-build check (check-pwa.mjs), so they cannot diverge.

/**
 * Shell JS = the scripts `index.html` loads (module entry + `modulepreload` static deps).
 * Every other emitted chunk is a lazy `import()` (Shiki grammars/themes/wasm, ...).
 * @param {string} html
 * @returns {Set<string>} paths relative to the site root, e.g. `assets/index-abc.js`
 */
export function shellScripts(html) {
  const urls = new Set();
  for (const [tag] of html.matchAll(/<(?:script|link)\b[^>]*>/g)) {
    const isScript = tag.startsWith("<script") && /\btype="module"/.test(tag);
    const isPreload = /\brel="modulepreload"/.test(tag);
    if (!isScript && !isPreload) continue;
    const m = /\b(?:src|href)="([^"]+)"/.exec(tag);
    if (m) urls.add(m[1].replace(/^\//, ""));
  }
  return urls;
}

/**
 * Whether a precache entry belongs to the shell: anything that is not a built `assets/*.js`
 * chunk, or an `assets/*.js` chunk that `index.html` loads.
 * @param {string} url
 * @param {Set<string>} shell result of {@link shellScripts}
 */
export function isShellEntry(url, shell) {
  return !(url.startsWith("assets/") && url.endsWith(".js")) || shell.has(url);
}

/** Known lazy Shiki pack chunk names: a belt-and-braces guard in the post-build check. */
export const SHIKI_PACK_NAME = /^assets\/(cpp|emacs-lisp|wasm|wolfram|typescript|javascript|tsx|jsx|vue-vine|angular-ts|objective-cpp|typst|mdx|dist)-/;

/** Shell budget enforced by the post-build check. */
export const SHELL_MAX_ENTRIES = 40;
export const SHELL_MAX_BYTES = Math.round(2.5 * 1024 * 1024);
