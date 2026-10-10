<script setup lang="ts">
import { RefreshCw } from "@lucide/vue";
import { toast } from "vue-sonner";
import { Button } from "@/components/ui/button";
import type { ChangesUpdate } from "@/core/types";
import { errorMessage } from "@/lib/errors";
import { useChangesStore } from "./store";

const props = defineProps<{ projectId: string }>();
const store = useChangesStore();

function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

function report(result: ChangesUpdate) {
  const n = result.conflicts.length;
  if (n > 0) {
    toast.warning(`Conflicts in ${plural(n, "change")}`, {
      description: "Those changes were left as they were.",
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
    :disabled="store.updating"
    @click="onGetLatest"
  >
    <RefreshCw class="size-3.5" :class="store.updating ? 'animate-spin' : ''" />
    {{ store.updating ? "Getting latest…" : "Get latest" }}
  </Button>
</template>
