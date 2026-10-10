<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Button } from "@/components/ui/button";
import type { ConflictChoice } from "@/core/types";
import ConflictFile from "./ConflictFile.vue";
import { useChangesStore } from "./store";

const props = defineProps<{ projectId: string }>();
const store = useChangesStore();
const selectedPath = ref<string | null>(null);
const confirmAbandon = ref(false);
const busy = ref(false);
const session = computed(() => store.currentConflict);
const file = computed(() => session.value?.files.find((f) => f.path === selectedPath.value) ?? session.value?.files[0]);

watch(session, (value) => { selectedPath.value = value?.files[0]?.path ?? null; confirmAbandon.value = false; }, { immediate: true });

async function choose(choice: ConflictChoice) {
  if (!file.value) return;
  busy.value = true;
  try { await store.chooseConflict(props.projectId, file.value.path, choice); }
  catch { /* Error shown from store. */ }
  finally { busy.value = false; }
}

async function finish() {
  busy.value = true;
  try { await store.finishConflict(props.projectId); }
  catch { /* Error shown from store. */ }
  finally { busy.value = false; }
}

async function abandon() {
  busy.value = true;
  try { await store.abandonConflict(props.projectId); }
  catch { /* Error shown from store. */ }
  finally { busy.value = false; confirmAbandon.value = false; }
}
</script>

<template>
  <div class="bg-background absolute inset-0 z-20 flex min-h-0 flex-col" data-testid="conflict-view">
    <header class="border-b px-4 py-3">
      <div class="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 class="text-lg font-semibold">Resolve changes</h2>
          <p class="text-muted-foreground text-sm">Your change and the latest main edited the same file. Review each choice before finishing.</p>
        </div>
        <div class="flex gap-2">
          <Button v-if="!confirmAbandon" variant="outline" size="sm" data-testid="abandon-conflict" @click="confirmAbandon = true">Abandon</Button>
          <Button v-else variant="destructive" size="sm" data-testid="confirm-abandon" :disabled="busy" @click="abandon">Confirm abandon</Button>
          <Button v-if="confirmAbandon" variant="ghost" size="sm" @click="confirmAbandon = false">Cancel</Button>
          <Button size="sm" data-testid="finish-conflict" :disabled="busy || !store.candidate?.complete" @click="finish">Finish update</Button>
        </div>
      </div>
      <p v-if="store.conflictError" class="text-destructive mt-2 text-sm" role="alert">{{ store.conflictError }}</p>
    </header>
    <div v-if="session" class="flex min-h-0 flex-1">
      <aside class="border-r w-60 shrink-0 overflow-auto p-3">
        <p class="text-muted-foreground mb-2 text-xs">{{ session.branch }}</p>
        <button v-for="item in session.files" :key="item.path" type="button" class="hover:bg-accent block w-full rounded px-2 py-2 text-left text-sm" :class="item.path === file?.path ? 'bg-accent' : ''" @click="selectedPath = item.path">
          <span class="block truncate">{{ item.path.split('/').at(-1) }}</span>
          <span class="text-muted-foreground block truncate text-xs">{{ item.path }}</span>
          <span v-if="session.choices[item.path]" class="text-xs">{{ session.choices[item.path] }}</span>
        </button>
      </aside>
      <div class="min-w-0 flex-1 overflow-auto p-4">
        <ConflictFile v-if="file" :file="file" :choice="session.choices[file.path]" @choose="choose" />
      </div>
    </div>
    <p v-else class="p-4 text-sm">Loading conflict…</p>
  </div>
</template>
