/** The absolute http(s) URL an anchor points at, or null for in-page/relative links. */
export function webLinkUrl(href: string, base: string): string | null {
  try {
    const url = new URL(href, base);
    return url.protocol === "http:" || url.protocol === "https:" ? url.href : null;
  } catch {
    return null;
  }
}

/**
 * Route a clicked web link out of the app: `window.open` reaches the desktop shell's
 * new-window handler, which opens it in the user's default browser (a daemon link in a
 * managed window); in a plain browser it opens a new tab. Without this the webview
 * navigates itself away from the app, or drops the click.
 */
export function openWebLink(e: MouseEvent): void {
  const anchor = (e.target as Element | null)?.closest?.("a[href]");
  if (!anchor) return;
  const url = webLinkUrl(anchor.getAttribute("href") ?? "", window.location.href);
  if (!url) return;
  // Capture phase, before the markdown renderer's own click handler: stop it showing
  // its safety dialog for a link that is already on its way to the browser.
  e.preventDefault();
  e.stopPropagation();
  window.open(url, "_blank", "noopener,noreferrer");
}
