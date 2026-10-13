import { toast } from "vue-sonner";
import type { Change, ChangesUpdate } from "@/core/types";
import { errorMessage } from "@workbench/utils";
import { useChangesStore } from "./store";

function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

function titles(list: Change[]): string {
  return list.map((c) => c.title).join(", ");
}

/** "Updated main and 2 changes (A, B)." — or undefined when nothing moved. */
function updatedSummary(result: ChangesUpdate): string | undefined {
  const parts: string[] = [];
  if (result.mainUpdated) parts.push("main");
  if (result.updated.length) {
    parts.push(`${plural(result.updated.length, "change")} (${titles(result.updated)})`);
  }
  return parts.length ? `Updated ${parts.join(" and ")}.` : undefined;
}

const DIVERGED =
  "Your local main has commits that are not on GitHub, so it was left as is. Ask a developer to bring it back in line with GitHub.";

function skippedNote(result: ChangesUpdate): string | undefined {
  return result.skipped ? "Your current change will update when the agent finishes." : undefined;
}

export function report(result: ChangesUpdate) {
  const summary = updatedSummary(result);
  const skipped = skippedNote(result);
  if (result.conflicts.length > 0) {
    const unsupported = result.conflictErrors && Object.entries(result.conflictErrors).map(([branch, reason]) => `${branch}: ${reason}`);
    const description = [
      `Left as they were: ${titles(result.conflicts)}.`,
      ...(unsupported ?? []),
      summary,
      result.mainDiverged ? DIVERGED : undefined,
      skipped,
    ]
      .filter(Boolean)
      .join(" ");
    toast.warning(`Conflicts in ${plural(result.conflicts.length, "change")}`, { description });
    return;
  }
  if (result.mainDiverged) {
    toast.warning("Changes updated, but main was left as is", {
      description: [DIVERGED, summary, skipped].filter(Boolean).join(" "),
    });
    return;
  }
  if (skipped) {
    toast.info("Syncing with main", { description: skipped });
    return;
  }
  toast.success("Synced with main", {
    description: result.updated.length
      ? `Merged main into ${plural(result.updated.length, "change")}.`
      : "Everything was already up to date.",
  });
}

/** Pull main, update every open change, and toast the outcome. */
export async function getLatest(projectId: string) {
  try {
    report(await useChangesStore().getLatest(projectId));
  } catch (e) {
    toast.error(errorMessage(e));
  }
}
