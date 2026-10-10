<script setup lang="ts">
import { RefreshCw } from "@lucide/vue";
import { toast } from "vue-sonner";
import { Button } from "@/components/ui/button";
import type { Change, ChangesUpdate } from "@/core/types";
import { errorMessage } from "@/lib/errors";
import { useChangesStore } from "./store";

const props = defineProps<{ projectId: string }>();
const store = useChangesStore();

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

function report(result: ChangesUpdate) {
  const summary = updatedSummary(result);
  if (result.conflicts.length > 0) {
    const description = [
      `Left as they were: ${titles(result.conflicts)}.`,
      summary,
      result.mainDiverged ? DIVERGED : undefined,
    ]
      .filter(Boolean)
      .join(" ");
    toast.warning(`Conflicts in ${plural(result.conflicts.length, "change")}`, { description });
    return;
  }
  if (result.mainDiverged) {
    toast.warning("Changes updated, but main was left as is", {
      description: [DIVERGED, summary].filter(Boolean).join(" "),
    });
    return;
  }
  toast.success("Up to date", {
    description: result.updated.length
      ? `Brought ${plural(result.updated.length, "change")} up to date with main.`
      : undefined,
  });
}

async function onGetLatest() {
  try {
    report(await store.getLatest(props.projectId));
  } catch (e) {
    toast.error(errorMessage(e));
  }
}
</script>

<template>
  <Button
    variant="ghost"
    size="sm"
    class="h-7 gap-1 px-2 text-xs"
    data-testid="get-latest"
    title="Pull main and bring every open change up to date"
    :disabled="store.updating || store.switching || store.creating"
    @click="onGetLatest"
  >
    <RefreshCw class="size-3.5" :class="store.updating ? 'animate-spin' : ''" />
    {{ store.updating ? "Getting latest…" : "Get latest" }}
  </Button>
</template>
