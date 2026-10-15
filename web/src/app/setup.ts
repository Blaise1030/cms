import type { SetupStatus } from "@/core/types";

const DISMISSED_KEY = "cms:setup-dismissed";

/** Something the app needs is missing: git, the GitHub CLI, a GitHub sign-in, or every agent. */
export function setupIncomplete(s: SetupStatus): boolean {
  return !s.git || !s.gh || !s.githubSignedIn || !s.providers.some((p) => p.available);
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
