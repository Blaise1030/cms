<script setup lang="ts">
import { computed, defineAsyncComponent, ref, watch } from "vue";
import { ArrowLeft, ChevronRight, GitCommitVertical, RotateCcw } from "@lucide/vue";
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
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Stepper,
  StepperIndicator,
  StepperItem,
  StepperSeparator,
} from "@/components/ui/stepper";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
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
const time = (ts: number) => new Date(ts * 1000).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
const day = (ts: number) => new Date(ts * 1000).toDateString();
function dayLabel(ts: number) {
  const today = new Date();
  if (day(ts) === today.toDateString()) return "Today";
  today.setDate(today.getDate() - 1);
  if (day(ts) === today.toDateString()) return "Yesterday";
  return new Date(ts * 1000).toLocaleDateString([], { weekday: "long", month: "long", day: "numeric", year: "numeric" });
}
/** A day heading goes above the newest checkpoint of each day. */
const startsDay = (i: number) => i === 0 || day(list.value[i]!.ts) !== day(list.value[i - 1]!.ts);

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
      <div class="flex min-h-10 shrink-0 items-center gap-3 px-2 py-1.5">
        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                variant="outline"
                size="icon-sm"
                class="self-start"
                aria-label="Back to timeline"
                @click="store.select(projectId, null)"
              >
                <ArrowLeft />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Back to timeline</TooltipContent>
          </Tooltip>
          <div class="min-w-0 flex-1 space-y-1">
            <p class="truncate text-base font-medium">{{ selected.summary }}</p>
            <div class="flex min-w-0 flex-wrap items-center gap-1.5">
              <Badge variant="secondary" class="max-w-40 truncate font-normal">
                {{ selected.author }}
              </Badge>
              <Badge variant="outline" class="font-mono font-normal">
                {{ short(selected.sha) }}
              </Badge>
              <Badge variant="outline" class="font-normal">
                {{ when(selected.ts) }}
              </Badge>
              <Badge v-if="isCurrent" variant="secondary" class="font-normal">Current</Badge>
            </div>
          </div>
          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                size="icon-sm"
                variant="outline"
                class="shrink-0"
                :disabled="isCurrent || store.restoring"
                :aria-label="isCurrent ? 'Current checkpoint' : 'Restore checkpoint'"
                data-testid="restore"
                @click="confirming = true"
              >
                <RotateCcw />
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {{ isCurrent ? "Current checkpoint" : "Restore checkpoint" }}
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>
      </div>
      <p v-if="store.diffLoading" class="text-muted-foreground p-3 text-sm">Loading diff…</p>
      <p v-else-if="store.diffError" class="text-destructive p-3 text-sm" role="alert">
        {{ store.diffError }}
      </p>
      <p
        v-else-if="store.diff && store.diff.files.length === 0"
        class="text-muted-foreground p-3 text-sm"
      >
        No file changes in this checkpoint.
      </p>
      <ResizablePanelGroup
        v-else-if="store.diff"
        direction="horizontal"
        auto-save-id="checkpoint-files-sidebar"
        class="min-h-0 flex-1"
      >
        <ResizablePanel id="checkpoint-files-tree" :default-size="20" :min-size="12" :max-size="45">
          <aside class="flex h-full flex-col overflow-hidden px-1 pt-1">
            <ChangedFilesTree fill :files="store.diff.files" @select="reveal" />
          </aside>
        </ResizablePanel>
        <ResizableHandle with-handle class="bg-transparent" />
        <ResizablePanel id="checkpoint-diffs" :default-size="80" :min-size="40">
          <ScrollArea class="bg-background h-full min-h-0">
            <div ref="diffs" class="flex flex-col gap-3 p-3">
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
            </div>
          </ScrollArea>
        </ResizablePanel>
      </ResizablePanelGroup>
    </template>

    <template v-else>
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
            <template v-for="(cp, i) in list" :key="cp.sha">
            <h3
              v-if="startsDay(i)"
              class="text-muted-foreground px-1 pb-1 text-xs font-medium"
              :class="{ 'pt-4': i > 0 }"
              data-testid="checkpoint-day"
            >
              {{ dayLabel(cp.ts) }}
            </h3>
            <StepperItem
              :step="i + 1"
              class="relative w-full items-start gap-2"
            >
              <!-- Nodes sit 13px down (the item's border and top padding) to line up with the title;
                   the line runs from 4px below this node to 4px above the next row's node. -->
              <StepperSeparator
                v-if="i < list.length - 1 && !startsDay(i + 1)"
                class="bg-border absolute top-[41px] -bottom-[9px] left-[11.5px] w-px"
              />
              <StepperIndicator
                class="bg-muted text-muted-foreground mt-[13px] size-6 shrink-0 group-data-[state=active]:bg-primary"
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
                  <ItemTitle class="w-full text-base">
                    <span class="truncate">{{ cp.summary }}</span>
                  </ItemTitle>
                  <ItemDescription class="text-xs">
                    <span data-testid="checkpoint-author">{{ cp.author }}</span> · {{ short(cp.sha) }} · {{ time(cp.ts) }}
                  </ItemDescription>
                </ItemContent>
                <ItemActions>
                  <Badge v-if="i === 0" variant="secondary">Current</Badge>
                  <ChevronRight class="text-muted-foreground size-4" />
                </ItemActions>
              </Item>
            </StepperItem>
            </template>
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
