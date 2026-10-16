/**
 * The macOS desktop shell announces a downloaded update by setting
 * `window.__WORKBENCH_UPDATE__` and dispatching this event on `window`. Browsers
 * and the PWA never see it.
 */
export const UPDATE_EVENT = "workbench:update-ready";

export interface DesktopUpdate {
  version: string;
  /** The shell's bundled update page, which asks before applying anything. */
  applyUrl: string;
}

declare global {
  interface Window {
    __WORKBENCH_UPDATE__?: unknown;
  }
}

/** Accepts only announcements pointing at the shell's own bundled page. */
export function parseUpdate(detail: unknown): DesktopUpdate | null {
  if (!detail || typeof detail !== "object") return null;
  const { version, applyUrl } = detail as Record<string, unknown>;
  if (typeof version !== "string" || typeof applyUrl !== "string") return null;
  try {
    const url = new URL(applyUrl);
    if (url.protocol !== "tauri:" || url.host !== "localhost") return null;
  } catch {
    return null;
  }
  return { version, applyUrl };
}
