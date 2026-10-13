/**
 * The address-bar path for a framed preview URL: the part after the project's base
 * (`/preview/{id}/about/?q=1#top` → `/about/?q=1#top`).
 */
export function framePath(href: string, base: string): string {
  const url = new URL(href, "http://frame.invalid");
  const prefix = base.replace(/\/$/, "");
  const path = url.pathname.startsWith(prefix) ? url.pathname.slice(prefix.length) : url.pathname;
  return `${path || "/"}${url.search}${url.hash}`;
}

/** The frame URL for something typed in the address bar (`about` or `/about` → `{base}about`). */
export function frameHref(input: string, base: string): string {
  const trimmed = input.trim().replace(/^\/+/, "");
  return `${base.replace(/\/?$/, "/")}${trimmed}`;
}
