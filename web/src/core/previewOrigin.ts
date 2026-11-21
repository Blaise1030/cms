import { frameHref } from "@orso/utils";

/**
 * Where the daemon serves a preview: the root of an origin of its own, a loopback port that
 * proxies the project's dev server (`GET /api/projects/{id}/preview`) or serves a built
 * conflict snapshot. `token` is that origin's first-load token; absent when the daemon runs
 * without auth.
 */
export interface PreviewAccess {
  port: number;
  token: string | null;
}

/**
 * `http://{daemon host}:{port}`. The daemon's own hostname (whatever the app reaches it by), so
 * the preview origin is same-site with the app and its `SameSite=Lax` cookie works in a frame.
 */
export function previewOrigin(access: Pick<PreviewAccess, "port">, daemonOrigin: string): string {
  let host = "127.0.0.1";
  try {
    host = new URL(daemonOrigin).hostname || host;
  } catch {
    // Keep loopback.
  }
  return `http://${host}:${access.port}`;
}

/**
 * Absolute URL of `path` (`about`, `/about/?q=1#top`) on the preview origin. `withToken` adds the
 * first-load token as `?t=`: the origin trades it for its own cookie and redirects to the same
 * address without it, so the page never sees it. Use it for loads the app starts (the frame's
 * `src`, a new tab), not for addresses shown to the reader.
 */
export function previewHref(
  access: PreviewAccess,
  daemonOrigin: string,
  path: string,
  withToken = false,
): string {
  const url = new URL(frameHref(path, "/"), `${previewOrigin(access, daemonOrigin)}/`);
  if (withToken && access.token) url.searchParams.set("t", access.token);
  return url.href;
}
