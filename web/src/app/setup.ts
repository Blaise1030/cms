import type { SetupStatus } from "@/core/types";

const DISMISSED_KEY = "cms:setup-dismissed";

/** Longest the shell waits for the first setup status before opening anyway. */
export const SETUP_WAIT_MS = 8000;

/** Something the app needs is missing: git, the GitHub CLI, a GitHub sign-in, the site builder, or every assistant. */
export function setupIncomplete(s: SetupStatus): boolean {
  return (
    !s.git ||
    !s.gh ||
    !s.githubSignedIn ||
    !s.providers.some((p) => p.chatReady) ||
    // Older daemons do not report the site builder; do not block on its absence.
    (s.node != null && s.node.phase !== "ready")
  );
}

export function setupDismissed(): boolean {
  try {
    return localStorage.getItem(DISMISSED_KEY) === "1";
  } catch {
    return false;
  }
}

export function dismissSetup() {
  try { localStorage.setItem(DISMISSED_KEY, "1"); } catch { /* storage blocked: the check shows again next time */ }
}
