import type { ChecksState, PrState, PrStatus } from "@/core/types";

/** Writer words for a shared change's state; git and GitHub terms stay out of the UI. */
export const PR_STATE_LABEL: Record<PrState, string> = {
  draft: "Draft",
  open: "Waiting for greenlight",
  approved: "Greenlit",
  changes_requested: "Sent back",
  merged: "Published",
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
