import type { ReviewCheck } from "@/core/types";

const PASSED = new Set(["success", "neutral", "skipped"]);
const FAILED = new Set(["failure", "timed_out", "cancelled", "action_required", "startup_failure", "stale"]);

/** GitHub's "✕ 8/9": how many checks passed, of how many, and the overall state. */
export function checkSummary(checks: ReviewCheck[]): {
  passed: number;
  total: number;
  state: "passing" | "failing" | "pending" | "none";
} {
  const passed = checks.filter((c) => c.status === "completed" && PASSED.has(c.conclusion ?? "")).length;
  const total = checks.length;
  const state =
    total === 0
      ? "none"
      : checks.some((c) => FAILED.has(c.conclusion ?? ""))
        ? "failing"
        : checks.some((c) => c.status !== "completed")
          ? "pending"
          : "passing";
  return { passed, total, state };
}

/** One check's state, for its icon. */
export function checkState(check: ReviewCheck): "passing" | "failing" | "pending" {
  if (check.status !== "completed") return "pending";
  return FAILED.has(check.conclusion ?? "") ? "failing" : "passing";
}
