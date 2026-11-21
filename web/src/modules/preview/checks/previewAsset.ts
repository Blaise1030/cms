/**
 * Page asset URLs for Page details. The preview runs at the root of its own origin, as the live
 * site does, so a site path is the same in the preview and in production.
 */

/**
 * Site-root path for Page Details labels: `href` resolved against the framed page, without the
 * host, so reviewers see `/og.png` instead of a preview- or host-qualified URL.
 */
export function previewSitePath(href: string, pageUrl?: string): string {
  const value = href.trim();
  if (!value) return "";
  try {
    const resolved = pageUrl ? new URL(value, pageUrl) : new URL(value);
    const path = `${resolved.pathname}${resolved.search}`.replace(/\/{2,}/g, "/");
    return path || "/";
  } catch {
    return value;
  }
}

/**
 * Rewrite a favicon / OG / similar href so it loads from the running preview: published absolute
 * URLs (CDN / SITE) and site-root paths (`/og.png`) are served by the same path on the framed
 * page's origin.
 */
export function localizePreviewAssetUrl(href: string, pageUrl: string): string {
  const value = href.trim();
  if (!value) return "";
  try {
    const page = new URL(pageUrl);
    const asset = new URL(value, pageUrl);
    if (asset.protocol !== "http:" && asset.protocol !== "https:") return value;
    if (asset.origin === page.origin) return asset.href;
    return new URL(`${asset.pathname}${asset.search}${asset.hash}`, page.origin).href;
  } catch {
    return value;
  }
}

/**
 * URL for an `<img src>` in the app: the asset on the preview origin. The app is same-site with
 * that origin, so the image request carries the cookie the frame's first load set there.
 */
export function previewMediaSrc(href: string, pageUrl: string): string {
  return localizePreviewAssetUrl(href, pageUrl);
}
