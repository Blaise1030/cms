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

/** "Updated your copy of the live site and 2 changes (A, B)." — or undefined when nothing moved. */
function updatedSummary(result: ChangesUpdate): string | undefined {
  const parts: string[] = [];
  if (result.mainUpdated) parts.push("your copy of the live site");
  if (result.updated.length) {
    parts.push(`${plural(result.updated.length, "change")} (${titles(result.updated)})`);
  }
  return parts.length ? `Updated ${parts.join(" and ")}.` : undefined;
}

const DIVERGED =
  "Your copy of the live site has edits that are not on GitHub, so it was left as is. Ask a developer to bring it back in line with GitHub.";

function skippedNote(result: ChangesUpdate): string | undefined {
  return result.skipped ? "Your current change will update when the agent finishes." : undefined;
}

export function report(result: ChangesUpdate) {
  if (result.conflicts.some((c) => result.conflictSessions?.[c.branch])) return;
  if (result.conflicts.length > 0) {
    toast.warning("Some drafts couldn't be updated", {
      description: "Try syncing again. If it keeps happening, ask your site admin.",
    });
    return;
  }
  const summary = updatedSummary(result);
  const skipped = skippedNote(result);
  if (result.mainDiverged) {
    toast.warning("Your draft is updated; the live site was left as is.", {
      description: [DIVERGED, summary, skipped].filter(Boolean).join(" "),
    });
    return;
  }
  if (skipped) {
    toast.info("Syncing with the live site", { description: skipped });
    return;
  }
  toast.success("Up to date with the live site", {
    description: result.updated.length
      ? `Brought the live site into ${plural(result.updated.length, "change")}.`
      : "Everything was already up to date.",
  });
}

/** Pull the live site, update every open change, and toast the outcome. */
export async function getLatest(projectId: string) {
  try {
    report(await useChangesStore().getLatest(projectId));
  } catch (e) {
    toast.error(errorMessage(e));
  }
}
