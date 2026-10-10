import type { ChecksState, PrState, PrStatus } from "@/core/types";

export const PR_STATE_LABEL: Record<PrState, string> = {
  draft: "Draft",
  open: "Open",
  approved: "Approved",
  merged: "Merged",
  closed: "Closed",
};

const CHECKS_LABEL: Record<Exclude<ChecksState, "none">, string> = {
  pending: "Checks pending",
  passing: "Checks passing",
  failing: "Checks failing",
};

/** "Draft · Checks pending"; checks are left out when there are none or the PR is done. */
export function prStatusLabel(s: Pick<PrStatus, "state" | "checks">): string {
  const state = PR_STATE_LABEL[s.state];
  if (s.checks === "none" || s.state === "merged" || s.state === "closed") return state;
  return `${state} · ${CHECKS_LABEL[s.checks]}`;
}
