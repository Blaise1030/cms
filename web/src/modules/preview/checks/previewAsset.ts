/** Map a page asset URL onto the live preview origin so the CMS can load it. */

/** `/preview/{projectId}/` prefix from a framed preview pathname, or the directory of the path. */
export function previewBasePath(pathname: string): string {
  const match = pathname.match(/^(\/preview\/[^/]+\/)/);
  if (match) return match[1]!;
  if (pathname.endsWith("/")) return pathname;
  const slash = pathname.lastIndexOf("/");
  return slash >= 0 ? pathname.slice(0, slash + 1) : "/";
}

/**
 * Site-root path for Page Details labels.
 * Resolves `href` against the framed page, then strips Orso's `/preview/{id}/` mount so
 * reviewers see `/og.png` instead of a preview- or host-qualified URL.
 */
export function previewSitePath(href: string, pageUrl?: string): string {
  const value = href.trim();
  if (!value) return "";
  try {
    const resolved = pageUrl ? new URL(value, pageUrl) : new URL(value);
    let path = `${resolved.pathname}${resolved.search}`;
    const fromPage = pageUrl ? previewBasePath(new URL(pageUrl).pathname) : "";
    const fromPath = path.match(/^(\/preview\/[^/]+\/)/)?.[1] ?? "";
    const mount = fromPage.startsWith("/preview/") ? fromPage : fromPath;
    if (mount && mount !== "/" && path.startsWith(mount)) {
      path = `/${path.slice(mount.length)}`;
    }
    path = path.replace(/\/{2,}/g, "/");
    return path || "/";
  } catch {
    return value.startsWith("/") ? value : value;
  }
}

/**
 * Rewrite a favicon / OG / similar href so it points at the running preview server.
 * Published absolute URLs (CDN / SITE) and site-root paths (`/og.png`) are mapped under
 * the framed origin's `/preview/{id}/` mount so the file loads from the local dev server.
 */
export function localizePreviewAssetUrl(href: string, pageUrl: string): string {
  const value = href.trim();
  if (!value) return "";
  try {
    const page = new URL(pageUrl);
    const base = previewBasePath(page.pathname);
    const asset = new URL(value, pageUrl);
    if (asset.protocol !== "http:" && asset.protocol !== "https:") return value;

    const path = asset.pathname;
    const underPreview =
      (base.startsWith("/preview/") && path.startsWith(base)) || path.startsWith("/preview/");

    // Already on the framed host and under the preview mount — keep as-is.
    if (asset.origin === page.origin && underPreview) {
      return asset.href;
    }

    // Site-root or foreign host: serve the same path from the local preview mount.
    const sitePath = underPreview
      ? path.slice(path.indexOf("/preview/")).replace(/^\/preview\/[^/]+/, "") || "/"
      : path;
    const relative = (sitePath.startsWith("/") ? sitePath.slice(1) : sitePath) + asset.search + asset.hash;
    return new URL(relative, page.origin + base).href;
  } catch {
    return value;
  }
}

export type PreviewMediaOpts = {
  /** Origin of the CMS UI (Vite or daemon shell), used for same-origin `/preview` loads. */
  uiOrigin: string;
  /** Daemon `?t=` / Bearer token so proxied preview assets authenticate. */
  token?: string;
};

/**
 * URL suitable for an `<img src>` in the CMS shell: localized onto the live preview,
 * then (when a daemon token is available) served same-origin via the UI `/preview`
 * proxy so the browser does not have to load the Astro loopback port directly.
 */
export function previewMediaSrc(
  href: string,
  pageUrl: string,
  opts: PreviewMediaOpts,
): string {
  const localized = localizePreviewAssetUrl(href, pageUrl);
  if (!localized) return "";
  if (!opts.token) return localized;
  try {
    const asset = new URL(localized);
    if (!asset.pathname.startsWith("/preview/")) return localized;
    const proxied = new URL(asset.pathname + asset.search + asset.hash, opts.uiOrigin);
    proxied.searchParams.set("t", opts.token);
    return proxied.href;
  } catch {
    return localized;
  }
}
