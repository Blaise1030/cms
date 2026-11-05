/** The recovery a failed send can offer, matching the cards in the transcript. */
export type RecoveryAction = "setup" | "signIn";

const SIGN_IN =
  /(log ?in|log ?on|sign ?in|signin|signed in|auth(entication|orization)?|credential|unauthori[sz]ed|api[- ]?key|access token|not logged|not signed)/i;
const SETUP =
  /(command not found|not found|no such file|cannot find|not installed|isn'?t installed|missing binary|missing executable|failed to (start|spawn|launch)|enoent|does not exist)/i;

/**
 * Map a pre-stream failure to the same action an in-chat assistant error offers.
 * The daemon's spawn messages can name a binary, so they stay out of the UI;
 * only the recovery is shown.
 */
export function spawnRecovery(message: string | null | undefined): RecoveryAction | null {
  const text = (message ?? "").trim();
  if (!text) return null;
  if (SIGN_IN.test(text)) return "signIn";
  if (SETUP.test(text)) return "setup";
  return null;
}
