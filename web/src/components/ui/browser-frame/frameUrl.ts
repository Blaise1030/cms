/** Only http(s) pages are framed; anything else (javascript:, data:, relative) is refused. */
export function isHttpUrl(url: string | null | undefined): boolean {
  if (!url) return false;
  try {
    const u = new URL(url);
    return (u.protocol === "http:" || u.protocol === "https:") && !!u.host;
  } catch {
    return false;
  }
}

/**
 * What the address bar holds, normalised to `/path?query#hash`. A full URL keeps only its path,
 * so a link pasted from either site opens the same page on both.
 */
export function normalizePath(input: string): string {
  const raw = input.trim();
  if (!raw) return "/";
  try {
    const u = isHttpUrl(raw) ? new URL(raw) : new URL(raw.startsWith("/") ? raw : `/${raw}`, "http://x");
    return `${u.pathname}${u.search}${u.hash}`;
  } catch {
    return "/";
  }
}

/**
 * `path` on `base`'s site: keeps `base`'s origin and any path prefix it is served under
 * (`https://x.test/docs/` + `/a` → `https://x.test/docs/a`). `null` for a non-http base.
 */
export function frameUrl(base: string | null | undefined, path: string): string | null {
  if (!base || !isHttpUrl(base)) return null;
  const u = new URL(base);
  const target = new URL(normalizePath(path), "http://x");
  u.pathname = `${u.pathname.replace(/\/+$/, "")}${target.pathname}`;
  u.search = target.search;
  u.hash = target.hash;
  return u.toString();
}
