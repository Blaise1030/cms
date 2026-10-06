<script setup lang="ts">
import { computed, defineAsyncComponent, ref, watch } from "vue";
import { ArrowLeft, ChevronRight, GitCommitVertical, History, RotateCcw } from "@lucide/vue";
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
import { Item, ItemActions, ItemContent, ItemDescription, ItemTitle } from "@/components/ui/item";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Stepper,
  StepperIndicator,
  StepperItem,
  StepperSeparator,
} from "@/components/ui/stepper";
import { errorMessage } from "@/lib/errors";
import { useCheckpointsStore } from "./store";

// Lazy: the Pierre renderers bring Shiki along, which must stay out of the precached app shell.
const ChangedFilesTree = defineAsyncComponent(() => import("./ChangedFilesTree.vue"));
const DiffView = defineAsyncComponent(() => import("./DiffView.vue"));

const props = defineProps<{ projectId: string }>();
const store = useCheckpointsStore();

const confirming = ref(false);
const diffs = ref<HTMLElement | null>(null);

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

/** Scroll a file's diff into view when it is picked in the changed-files tree. */
function reveal(path: string) {
  const section = [...(diffs.value?.querySelectorAll<HTMLElement>("section[data-path]") ?? [])].find(
    (el) => el.dataset.path === path,
  );
  section?.scrollIntoView({ behavior: "smooth", block: "start" });
}

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
      <div class="flex h-10 shrink-0 items-center justify-between gap-2 px-3">
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
        <div ref="diffs" class="flex flex-col gap-4 p-3">
          <div>
            <p class="text-sm font-medium">{{ selected.summary }}</p>
            <p class="text-muted-foreground text-xs">{{ selected.author }} · {{ short(selected.sha) }} · {{ when(selected.ts) }}</p>
          </div>
          <p v-if="store.diffLoading" class="text-muted-foreground text-sm">Loading diff…</p>
          <p v-else-if="store.diffError" class="text-destructive text-sm" role="alert">
            {{ store.diffError }}
          </p>
          <template v-else-if="store.diff">
            <p v-if="store.diff.files.length === 0" class="text-muted-foreground text-sm">
              No file changes in this checkpoint.
            </p>
            <template v-else>
              <ChangedFilesTree :files="store.diff.files" @select="reveal" />
              <section
                v-for="file in store.diff.files"
                :key="file.path"
                :data-path="file.path"
                class="flex scroll-mt-3 flex-col gap-1.5"
              >
                <template v-if="file.binary">
                  <div class="flex items-center justify-between gap-2 font-mono text-xs">
                    <span class="truncate">{{ file.path }}</span>
                  </div>
                  <p class="text-muted-foreground text-xs">Binary file changed.</p>
                </template>
                <DiffView v-else :diff="file.meta" />
              </section>
            </template>
          </template>
        </div>
      </ScrollArea>
    </template>

    <template v-else>
      <div class="flex h-10 shrink-0 items-center gap-2 px-3 text-sm font-medium">
        <History class="size-4" /> Checkpoints
      </div>
      <ScrollArea class="min-h-0 flex-1">
        <div class="p-3">
          <p v-if="store.loading && list.length === 0" class="text-muted-foreground text-sm">Loading…</p>
          <p v-else-if="store.error" class="text-destructive text-sm" role="alert">{{ store.error }}</p>
          <p v-else-if="list.length === 0" class="text-muted-foreground text-sm">
            No checkpoints yet. One is saved after each agent turn that changes files.
          </p>
          <!-- Newest first; the current checkpoint is the active step. Non-linear so every step
               stays clickable (linear steppers disable steps past the next one). -->
          <Stepper
            v-else
            orientation="vertical"
            :model-value="1"
            :linear="false"
            class="flex w-full flex-col gap-0"
            data-testid="timeline"
          >
            <StepperItem
              v-for="(cp, i) in list"
              :key="cp.sha"
              :step="i + 1"
              class="relative w-full items-center gap-2"
            >
              <!-- Runs from just below this node to just above the next one: rows are equal
                   height (single-line title and meta), so the next centre is one row down. -->
              <StepperSeparator
                v-if="i < list.length - 1"
                class="bg-border absolute top-[calc(50%+16px)] bottom-[calc(16px-50%)] left-[11.5px] w-px"
              />
              <StepperIndicator
                class="bg-muted text-muted-foreground size-6 shrink-0 group-data-[state=active]:bg-primary"
              >
                <GitCommitVertical class="size-3.5" />
              </StepperIndicator>
              <Item
                as="button"
                type="button"
                size="sm"
                class="hover:bg-accent/60 min-w-0 flex-1 flex-nowrap px-2.5 text-left"
                data-testid="checkpoint-item"
                @click="store.select(projectId, cp.sha)"
              >
                <ItemContent class="min-w-0 gap-0.5">
                  <ItemTitle class="w-full">
                    <span class="truncate">{{ cp.summary }}</span>
                  </ItemTitle>
                  <ItemDescription class="text-xs">
                    <span data-testid="checkpoint-author">{{ cp.author }}</span> · {{ short(cp.sha) }} · {{ when(cp.ts) }}
                  </ItemDescription>
                </ItemContent>
                <ItemActions>
                  <Badge v-if="i === 0" variant="secondary">Current</Badge>
                  <ChevronRight class="text-muted-foreground size-4" />
                </ItemActions>
              </Item>
            </StepperItem>
          </Stepper>
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
