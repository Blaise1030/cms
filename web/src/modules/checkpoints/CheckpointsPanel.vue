<script setup lang="ts">
import { computed, defineAsyncComponent, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { toast } from "@/lib/toast";
import { historyPath } from "@/router";
import type { SplitterItem } from "@nuxt/ui";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UAvatar from "@nuxt/ui/components/Avatar.vue";
import UBadge from "@nuxt/ui/components/Badge.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UEmpty from "@nuxt/ui/components/Empty.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import UModal from "@nuxt/ui/components/Modal.vue";
import USeparator from "@nuxt/ui/components/Separator.vue";
import USplitter from "@nuxt/ui/components/Splitter.vue";
import UTooltip from "@nuxt/ui/components/Tooltip.vue";
import { errorMessage } from "@orso/utils";
import { useCheckpointsStore } from "./store";

// Lazy: the Pierre renderers bring Shiki along, which must stay out of the precached app shell.
const ChangedFilesTree = defineAsyncComponent(() => import("./ChangedFilesTree.vue"));
const DiffView = defineAsyncComponent(() => import("./DiffView.vue"));

const props = defineProps<{ projectId: string; sha?: string }>();
const store = useCheckpointsStore();
const router = useRouter();

const confirming = ref(false);
const diffs = ref<HTMLElement | null>(null);

const list = computed(() => store.byProject[props.projectId] ?? []);
const selected = computed(() => list.value.find((c) => c.sha === store.selectedSha) ?? null);
const isCurrent = computed(() => !!selected.value && list.value[0]?.sha === selected.value.sha);

watch(
  () => props.projectId,
  (id) => void store.load(id),
  { immediate: true },
);
// The URL names the open checkpoint; without one, the timeline shows.
watch(
  () => [props.projectId, props.sha ?? null] as const,
  ([id, sha]) => void store.select(id, sha),
  { immediate: true },
);

const open = (sha?: string) => router.push(historyPath(props.projectId, sha));

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

const SPLIT: SplitterItem[] = [
  { id: "checkpoint-files-tree", slot: "tree", defaultSize: 20, minSize: 12, maxSize: 45 },
  { id: "checkpoint-diffs", slot: "diffs", defaultSize: 80, minSize: 40 },
];

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
    void router.replace(historyPath(props.projectId));
    toast.success(`Restored ${short(target.sha)}`);
  } catch (e) {
    toast.error(`Restore failed: ${errorMessage(e)}`);
  } finally {
    confirming.value = false;
  }
}
</script>

<template>
  <div class="relative flex h-full min-h-0 flex-col" data-testid="checkpoints-panel">
    <template v-if="selected">
      <div class="flex min-h-10 shrink-0 items-center gap-3 px-2 py-1.5">
        <UTooltip text="Back to timeline">
          <UButton
            color="neutral"
            variant="outline"
            size="sm"
            icon="i-hugeicons-arrow-left-01"
            class="self-start"
            aria-label="Back to timeline"
            @click="open()"
          />
        </UTooltip>
        <div class="min-w-0 flex-1 space-y-1">
          <p class="text-highlighted truncate text-base font-medium">{{ selected.summary }}</p>
          <div class="flex min-w-0 flex-wrap items-center gap-1.5">
            <UBadge color="neutral" variant="soft" class="max-w-40 truncate" :label="selected.author" />
            <UBadge color="neutral" variant="outline" class="font-mono" :label="short(selected.sha)" />
            <UBadge color="neutral" variant="outline" :label="when(selected.ts)" />
            <UBadge v-if="isCurrent" color="neutral" variant="soft" label="Current" />
          </div>
        </div>
        <UTooltip :text="isCurrent ? 'Current checkpoint' : 'Restore checkpoint'">
          <UButton
            color="neutral"
            variant="outline"
            size="sm"
            icon="i-hugeicons-arrow-turn-backward"
            class="shrink-0"
            :disabled="isCurrent || store.restoring"
            :aria-label="isCurrent ? 'Current checkpoint' : 'Restore checkpoint'"
            data-testid="restore"
            @click="confirming = true"
          />
        </UTooltip>
      </div>
      <p v-if="store.diffLoading" class="text-muted flex items-center gap-2 p-3 text-sm">
        <UIcon name="i-hugeicons-loading-03" class="size-4 animate-spin" />
        Loading diff…
      </p>
      <div v-else-if="store.diffError" class="p-3">
        <UAlert
          color="error"
          variant="subtle"
          icon="i-hugeicons-alert-circle"
          :description="store.diffError"
          role="alert"
        />
      </div>
      <UEmpty
        v-else-if="store.diff && store.diff.files.length === 0"
        variant="naked"
        icon="i-hugeicons-file-validation"
        title="No file changes in this checkpoint."
      />
      <USplitter
        v-else-if="store.diff"
        :items="SPLIT"
        auto-save-id="checkpoint-files-sidebar"
        class="min-h-0 flex-1"
        :ui="{ handle: 'flex items-center justify-center' }"
      >
        <template #resize-handle>
          <div
            class="bg-accented group-hover:bg-inverted/70 group-data-[resize-handle-state=drag]:bg-inverted h-8 w-1 rounded-full transition-colors"
            aria-hidden="true"
          />
        </template>
        <template #tree>
          <aside class="flex h-full w-full flex-col overflow-hidden px-1 pt-1">
            <ChangedFilesTree fill :files="store.diff.files" @select="reveal" />
          </aside>
        </template>
        <template #diffs>
          <div class="bg-background h-full min-h-0 w-full overflow-y-auto">
            <div ref="diffs" class="flex flex-col gap-3 p-3">
              <section
                v-for="file in store.diff.files"
                :key="file.path"
                :data-path="file.path"
                class="flex scroll-mt-3 flex-col gap-1.5"
              >
                <template v-if="file.binary">
                  <div class="text-default flex items-center justify-between gap-2 font-mono text-xs">
                    <span class="truncate">{{ file.path }}</span>
                  </div>
                  <p class="text-muted text-xs">Binary file changed.</p>
                </template>
                <DiffView v-else :diff="file.meta" />
              </section>
            </div>
          </div>
        </template>
      </USplitter>
    </template>

    <template v-else>
      <div class="pointer-events-none absolute inset-x-0 top-0 z-10 h-6 bg-linear-to-b from-background to-transparent" aria-hidden="true" />
      <div class="min-h-0 flex-1 overflow-y-auto">
        <div class="p-3">
          <p v-if="store.loading && list.length === 0" class="text-muted flex items-center gap-2 text-sm">
            <UIcon name="i-hugeicons-loading-03" class="size-4 animate-spin" />
            Loading checkpoints…
          </p>
          <UAlert
            v-else-if="store.error"
            color="error"
            variant="subtle"
            icon="i-hugeicons-alert-circle"
            :description="store.error"
            role="alert"
          />
          <UEmpty
            v-else-if="list.length === 0"
            variant="naked"
            icon="i-hugeicons-clock-04"
            title="No checkpoints yet"
            description="One is saved after each assistant turn that changes files."
          />
          <!-- Newest first; the current checkpoint's node is highlighted. -->
          <ol v-else class="m-0 flex w-full list-none flex-col gap-2 p-0" data-testid="timeline">
            <template v-for="(cp, i) in list" :key="cp.sha">
              <li v-if="startsDay(i)" class="px-1 pb-1" :class="{ 'pt-4': i > 0 }">
                <h3 class="text-muted m-0 text-xs font-medium" data-testid="checkpoint-day">{{ dayLabel(cp.ts) }}</h3>
              </li>
              <li class="relative flex w-full items-start gap-2">
                <!-- The line runs from below this node, across the row gap, to the next row's node. -->
                <USeparator
                  v-if="i < list.length - 1 && !startsDay(i + 1)"
                  orientation="vertical"
                  class="absolute top-10 -bottom-4 left-3 h-auto w-px"
                  aria-hidden="true"
                />
                <UAvatar
                  icon="i-hugeicons-git-commit"
                  size="xs"
                  class="mt-3"
                  :class="i === 0 ? 'bg-primary text-inverted' : 'bg-elevated'"
                  :ui="i === 0 ? { icon: 'text-inverted' } : undefined"
                  aria-hidden="true"
                />
                <UButton
                  color="neutral"
                  variant="ghost"
                  class="min-w-0 flex-1 gap-2 px-2.5 py-2.5 text-left"
                  data-testid="checkpoint-item"
                  @click="open(cp.sha)"
                >
                  <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                    <span class="text-highlighted truncate text-base font-medium">{{ cp.summary }}</span>
                    <span class="text-muted text-xs font-normal">
                      <span data-testid="checkpoint-author">{{ cp.author }}</span> · {{ short(cp.sha) }} · {{ time(cp.ts) }}
                    </span>
                  </span>
                  <span class="flex shrink-0 items-center gap-2">
                    <UBadge v-if="i === 0" color="neutral" variant="soft" label="Current" />
                    <UIcon name="i-hugeicons-arrow-right-01" class="text-muted size-4" />
                  </span>
                </UButton>
              </li>
            </template>
          </ol>
        </div>
      </div>
    </template>

    <UModal
      v-model:open="confirming"
      title="Restore this checkpoint?"
      :description="`The project's files go back to ${selected ? short(selected.sha) : ''}. Later checkpoints and any uncommitted changes are discarded.`"
      :close="false"
      :ui="{ footer: 'justify-end' }"
    >
      <template #footer>
        <UButton color="neutral" variant="outline" label="Cancel" @click="confirming = false" />
        <UButton
          color="error"
          :loading="store.restoring"
          :label="store.restoring ? 'Restoring…' : 'Restore'"
          data-testid="confirm-restore"
          @click="restore"
        />
      </template>
    </UModal>
  </div>
</template>
