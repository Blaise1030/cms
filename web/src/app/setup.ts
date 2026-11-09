import type { SetupStatus } from "@/core/types";

const DISMISSED_KEY = "cms:setup-dismissed";

/** Longest the shell waits for the first setup status before opening anyway. */
export const SETUP_WAIT_MS = 8000;

/**
 * Required first-run pieces still missing: a chat-ready assistant, and the site
 * builder when the daemon reports one. GitHub stays optional (sign-in later).
 */
export function setupIncomplete(s: SetupStatus): boolean {
  return (
    !s.providers.some((p) => p.chatReady) ||
    // Older daemons do not report the site builder; do not block on its absence.
    (s.node != null && s.node.phase !== "ready" && s.node.phase !== "updateAvailable")
  );
}

/** Which onboarding step to open when some work is already done. */
export function onboardingStartStep(s: {
  chatReady: boolean;
  siteReady: boolean;
}): 1 | 2 | 3 {
  if (!s.chatReady) return 1;
  if (!s.siteReady) return 2;
  return 3;
}

export function setupDismissed(): boolean {
  try {
    return localStorage.getItem(DISMISSED_KEY) === "1";
  } catch {
    return false;
  }
}

export function dismissSetup() {
  try {
    localStorage.setItem(DISMISSED_KEY, "1");
  } catch {
    /* storage blocked: the check shows again next time unless the daemon flag sticks */
  }
}
