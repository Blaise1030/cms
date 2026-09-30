<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ArrowLeft, History, RotateCcw } from "@lucide/vue";
import { toast } from "vue-sonner";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { errorMessage } from "@/lib/errors";
import DiffView from "./DiffView.vue";
import { useCheckpointsStore } from "./store";

const props = defineProps<{ projectId: string }>();
const store = useCheckpointsStore();

const confirming = ref(false);

const list = computed(() => store.byProject[props.projectId] ?? []);
const selected = computed(() => list.value.find((c) => c.sha === store.selectedSha) ?? null);
const isCurrent = computed(() => !!selected.value && list.value[0]?.sha === selected.value.sha);

watch(
  () => props.projectId,
  (id) => {
    void store.select(id, null);
    void store.load(id);
  },
  { immediate: true },
);

const short = (sha: string) => sha.slice(0, 7);
const when = (ts: number) => new Date(ts * 1000).toLocaleString();

async function restore() {
  const target = selected.value;
  if (!target) return;
  try {
    await store.restore(props.projectId, target.sha);
    toast.success(`Restored ${short(target.sha)}`);
  } catch (e) {
    toast.error(`Restore failed: ${errorMessage(e)}`);
  } finally {
    confirming.value = false;
  }
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col" data-testid="checkpoints-panel">
    <template v-if="selected">
      <div class="flex h-10 shrink-0 items-center justify-between gap-2 border-b px-3">
        <Button variant="ghost" size="sm" @click="store.select(projectId, null)">
          <ArrowLeft /> Timeline
        </Button>
        <Button
          size="sm"
          variant="outline"
          :disabled="isCurrent || store.restoring"
          data-testid="restore"
          @click="confirming = true"
        >
          <RotateCcw /> {{ isCurrent ? "Current" : "Restore" }}
        </Button>
      </div>
      <ScrollArea class="min-h-0 flex-1">
        <div class="flex flex-col gap-4 p-3">
          <div>
            <p class="text-sm font-medium">{{ selected.summary }}</p>
            <p class="text-muted-foreground text-xs">{{ short(selected.sha) }} · {{ when(selected.ts) }}</p>
          </div>
          <p v-if="store.diffLoading" class="text-muted-foreground text-sm">Loading diff…</p>
          <p v-else-if="store.diffError" class="text-destructive text-sm" role="alert">
            {{ store.diffError }}
          </p>
          <template v-else-if="store.diff">
            <p v-if="store.diff.files.length === 0" class="text-muted-foreground text-sm">
              No file changes in this checkpoint.
            </p>
            <section v-for="file in store.diff.files" :key="file.path" class="flex flex-col gap-1.5">
              <div class="flex items-center justify-between gap-2 font-mono text-xs">
                <span class="truncate">{{ file.path }}</span>
                <span class="shrink-0">
                  <span class="text-emerald-500">+{{ file.added }}</span>
                  <span class="text-destructive ml-1">-{{ file.removed }}</span>
                </span>
              </div>
              <p v-if="file.binary" class="text-muted-foreground text-xs">Binary file changed.</p>
              <DiffView v-else :old-text="file.oldText" :new-text="file.newText" />
            </section>
          </template>
        </div>
      </ScrollArea>
    </template>

    <template v-else>
      <div class="flex h-10 shrink-0 items-center gap-2 border-b px-3 text-sm font-medium">
        <History class="size-4" /> Checkpoints
      </div>
      <ScrollArea class="min-h-0 flex-1">
        <div class="p-3">
          <p v-if="store.loading && list.length === 0" class="text-muted-foreground text-sm">Loading…</p>
          <p v-else-if="store.error" class="text-destructive text-sm" role="alert">{{ store.error }}</p>
          <p v-else-if="list.length === 0" class="text-muted-foreground text-sm">
            No checkpoints yet. One is saved after each agent turn that changes files.
          </p>
          <ol v-else class="border-border relative ml-2 border-l" data-testid="timeline">
            <li v-for="(cp, i) in list" :key="cp.sha" class="relative pb-4 pl-4 last:pb-0">
              <span class="bg-primary absolute top-2 -left-[4.5px] size-2 rounded-full" />
              <button
                type="button"
                class="hover:bg-accent w-full rounded-md p-2 text-left"
                data-testid="checkpoint-item"
                @click="store.select(projectId, cp.sha)"
              >
                <div class="flex items-center justify-between gap-2">
                  <span class="truncate text-sm font-medium">{{ cp.summary }}</span>
                  <Badge v-if="i === 0" variant="secondary">Current</Badge>
                </div>
                <span class="text-muted-foreground text-xs">{{ short(cp.sha) }} · {{ when(cp.ts) }}</span>
              </button>
            </li>
          </ol>
        </div>
      </ScrollArea>
    </template>

    <AlertDialog v-model:open="confirming">
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Restore this checkpoint?</AlertDialogTitle>
          <AlertDialogDescription>
            The project's files go back to {{ selected ? short(selected.sha) : "" }}. Later
            checkpoints and any uncommitted changes are discarded.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <Button variant="destructive" :disabled="store.restoring" data-testid="confirm-restore" @click="restore">
            {{ store.restoring ? "Restoring…" : "Restore" }}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  </div>
</template>
