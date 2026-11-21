<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { useDaemon } from "@/core/daemon";
import { toast } from "@/lib/toast";
import { historyPath } from "@/router";
import type { SplitterItem } from "@nuxt/ui";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UAvatar from "@nuxt/ui/components/Avatar.vue";
import UBadge from "@nuxt/ui/components/Badge.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UEmpty from "@nuxt/ui/components/Empty.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import UInput from "@nuxt/ui/components/Input.vue";
import UModal from "@nuxt/ui/components/Modal.vue";
import USeparator from "@nuxt/ui/components/Separator.vue";
import USplitter from "@nuxt/ui/components/Splitter.vue";
import UTooltip from "@nuxt/ui/components/Tooltip.vue";
import { errorMessage } from "@orso/utils";
import { isMacDesktop } from "@/lib/desktop";
import type { DeployState } from "@/core/types";
import { useCheckpointsStore } from "./store";

// Lazy: the Pierre renderers bring Shiki along, which must stay out of the precached app shell.
const ChangedFilesTree = defineAsyncComponent(() => import("./ChangedFilesTree.vue"));
const DiffView = defineAsyncComponent(() => import("./DiffView.vue"));

const props = defineProps<{ projectId: string; sha?: string }>();
const short = (sha: string) => sha.slice(0, 7);
const store = useCheckpointsStore();
const daemon = useDaemon();
const router = useRouter();

const confirming = ref(false);
/** No top bar on this tab: on the macOS desktop app the traffic lights sit over the top-left. */
const macDesktop = isMacDesktop();
const diffs = ref<HTMLElement | null>(null);

const list = computed(() => store.byProject[props.projectId] ?? []);
const selected = computed(() => list.value.find((c) => c.sha === store.selectedSha) ?? null);
/** The folder the timeline covers; `.` is the whole repo. */
const root = computed(() => store.rootByProject[props.projectId] ?? ".");
/** The commit the project is at. A site-scoped timeline may not list it (it changed only files outside the site). */
const currentSha = computed(() => {
  const head = store.headByProject[props.projectId];
  return head === undefined ? list.value[0]?.sha : head;
});
/** The timeline's search (summary, author or sha) and publish filter. */
const search = ref("");
type PublishFilter = "all" | "published" | "unpublished";
const publish = ref<PublishFilter>("all");
const PUBLISH_FILTERS: { id: PublishFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "unpublished", label: "In this change" },
  { id: "published", label: "Live" },
];
/** Only a change mixes its own checkpoints with live ones; on `main` everything is live. */
const showPublishFilter = computed(() => list.value.some((c) => c.published === false));
const shown = computed(() => {
  const needle = search.value.trim().toLowerCase();
  return list.value.filter(
    (c) =>
      (!needle ||
        c.summary.toLowerCase().includes(needle) ||
        c.author.toLowerCase().includes(needle) ||
        c.sha.startsWith(needle)) &&
      (publish.value === "all" ||
        !showPublishFilter.value ||
        (publish.value === "published") === (c.published === true)),
  );
});
/**
 * Each checkpoint's deploy, by sha: its site once live, or whether it is still building or
 * failed. Looked up for the newest published checkpoints in view and for the open one (a few
 * GitHub calls each). A change can be published before its site has deployed, so a building
 * deploy is asked about again until it is live or failed.
 */
type Deploy = { url: string | null; state: DeployState };
const deploys = ref<Record<string, Deploy>>({});
const DEPLOY_LOOKUPS = 15;
const DEPLOY_POLL_MS = 20_000;
async function lookUpDeploy(sha: string, again = false) {
  if (sha.length !== 40 || (!again && sha in deploys.value)) return;
  if (!again) deploys.value[sha] = { url: null, state: "unknown" };
  const project = props.projectId;
  const link = await daemon.api.deployLink(project, sha).catch(() => null);
  if (project !== props.projectId || !link) return;
  const url = link.url && /^https?:\/\//i.test(link.url) ? link.url : null;
  deploys.value[sha] = { url, state: url ? "live" : link.state };
}
watch(
  () => shown.value.filter((c) => c.published).slice(0, DEPLOY_LOOKUPS).map((c) => c.sha),
  (shas) => shas.forEach((sha) => void lookUpDeploy(sha)),
  { immediate: true },
);
watch(
  () => props.projectId,
  () => (deploys.value = {}),
);
const pollTimer = setInterval(() => {
  for (const [sha, d] of Object.entries(deploys.value)) if (d.state === "building") void lookUpDeploy(sha, true);
}, DEPLOY_POLL_MS);
onBeforeUnmount(() => clearInterval(pollTimer));
const deployOf = (sha: string): Deploy | undefined => deploys.value[sha];
/** Open a deployed site in the browser (the desktop shell hands `window.open` to it). */
function openDeploy(e: MouseEvent) {
  const href = (e.currentTarget as HTMLAnchorElement | null)?.href ?? "";
  if (!/^https?:\/\//i.test(href)) return;
  e.preventDefault();
  window.open(href, "_blank", "noopener,noreferrer");
}
const isCurrent = computed(() => !!selected.value && currentSha.value === selected.value.sha);
const restoreDescription = computed(() => {
  const target = selected.value ? short(selected.value.sha) : "";
  const base = `The project's files go back to ${target}. Later checkpoints and any uncommitted changes are discarded.`;
  return root.value === "."
    ? base
    : `${base} This resets the whole repository, including files outside ${root.value}/.`;
});

watch(
  () => selected.value?.sha,
  (sha) => sha && void lookUpDeploy(sha),
);
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
const startsDay = (i: number) => i === 0 || day(shown.value[i]!.ts) !== day(shown.value[i - 1]!.ts);

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
      <div
        :class="['flex min-h-10 shrink-0 items-center gap-3 px-2 py-1.5', macDesktop && 'pt-2 pl-10 fullscreen:pl-3']"
        :data-tauri-drag-region="macDesktop ? 'deep' : undefined"
      >
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
        <UButton
          v-if="deployOf(selected.sha)?.url"
          color="neutral"
          variant="outline"
          size="sm"
          icon="i-hugeicons-globe-02"
          trailing-icon="i-hugeicons-link-square-02"
          :ui="{ trailingIcon: 'size-3.5 opacity-60' }"
          label="Open deployed site"
          :to="deployOf(selected.sha)!.url!"
          target="_blank"
          class="shrink-0"
          data-testid="checkpoint-deploy"
          @click="openDeploy"
        />
        <UBadge
          v-else-if="deployOf(selected.sha)?.state === 'building'"
          color="neutral"
          variant="outline"
          icon="i-hugeicons-loading-03"
          label="Deploying…"
          :ui="{ leadingIcon: 'animate-spin' }"
          class="shrink-0"
          data-testid="checkpoint-deploy-building"
        />
        <UBadge
          v-else-if="deployOf(selected.sha)?.state === 'failed'"
          color="error"
          variant="subtle"
          icon="i-hugeicons-alert-circle"
          label="Deploy failed"
          class="shrink-0"
          data-testid="checkpoint-deploy-failed"
        />
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
      <!-- No project top bar here: like the Review inbox, the header clears the macOS traffic
           lights and is where the window is dragged from. The list scrolls beneath it. -->
      <div
        :class="['flex shrink-0 flex-col gap-2 px-3 pb-1', macDesktop ? 'pt-2 fullscreen:pt-3' : 'pt-3']"
        :data-tauri-drag-region="macDesktop ? 'deep' : undefined"
        data-testid="history-toolbar"
      >
        <header
          :class="['flex flex-col gap-0.5', macDesktop && 'pl-7 fullscreen:pl-0']"
          :data-tauri-drag-region="macDesktop ? 'deep' : undefined"
        >
          <h1 class="text-highlighted text-lg font-semibold">History</h1>
          <!-- A site folder is set in orso.json: only commits touching it are listed. -->
          <p v-if="root !== '.'" class="text-muted truncate text-sm" data-testid="history-scope">Changes to {{ root }}/</p>
        </header>
        <template v-if="list.length > 0">
          <UInput
            v-model="search"
            icon="i-hugeicons-search-01"
            size="md"
            class="w-full"
            placeholder="Search by summary, author or commit"
            aria-label="Search checkpoints"
            spellcheck="false"
            autocomplete="off"
            data-testid="history-search"
          />
          <div v-if="showPublishFilter" class="flex items-center gap-1" role="group" aria-label="Show">
            <UButton
              v-for="f in PUBLISH_FILTERS"
              :key="f.id"
              color="neutral"
              :variant="publish === f.id ? 'subtle' : 'ghost'"
              :class="publish === f.id ? 'text-highlighted' : 'text-muted'"
              size="sm"
              :label="f.label"
              :aria-pressed="publish === f.id"
              :data-testid="`history-filter-${f.id}`"
              @click="publish = f.id"
            />
          </div>
        </template>
      </div>
      <div class="relative min-h-0 flex-1">
      <div class="pointer-events-none absolute inset-x-0 top-0 z-10 h-6 bg-linear-to-b from-background to-transparent" aria-hidden="true" />
      <div class="h-full overflow-y-auto">
        <div class="px-3 pt-1 pb-3">
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
          <template v-else>
            <p v-if="shown.length === 0" class="text-muted m-0 px-1 py-6 text-center text-sm" data-testid="history-no-match">
              No checkpoints match.
            </p>
            <!-- Newest first; the current checkpoint's node is highlighted. -->
            <ol class="m-0 flex w-full list-none flex-col gap-2 p-0" data-testid="timeline">
              <template v-for="(cp, i) in shown" :key="cp.sha">
                <li v-if="startsDay(i)" class="px-1 pb-1" :class="{ 'pt-4': i > 0 }">
                  <h3 class="text-muted m-0 text-xs font-medium" data-testid="checkpoint-day">{{ dayLabel(cp.ts) }}</h3>
                </li>
                <li class="relative flex w-full items-start gap-2">
                  <!-- The line runs from below this node, across the row gap, to the next row's node. -->
                  <USeparator
                    v-if="i < shown.length - 1 && !startsDay(i + 1)"
                    orientation="vertical"
                    class="absolute top-10 -bottom-4 left-3 h-auto w-px"
                    aria-hidden="true"
                  />
                  <UAvatar
                    icon="i-hugeicons-git-commit"
                    size="xs"
                    class="mt-3"
                    :class="cp.sha === currentSha ? 'bg-primary text-inverted' : 'bg-elevated'"
                    :ui="cp.sha === currentSha ? { icon: 'text-inverted' } : undefined"
                    aria-hidden="true"
                  />
                  <!-- One card per checkpoint. Its button stretches over the whole card, so a click
                       anywhere opens it; the deployed-site link sits above that, inside the card. -->
                  <div
                    class="border-default bg-default hover:bg-elevated/50 has-focus-visible:ring-primary relative flex min-w-0 flex-1 items-center gap-2 rounded-lg border px-3 py-2.5 has-focus-visible:ring-2"
                  >
                    <button
                      type="button"
                      class="flex min-w-0 flex-1 cursor-pointer flex-col gap-0.5 text-left outline-none after:absolute after:inset-0 after:rounded-lg"
                      data-testid="checkpoint-item"
                      @click="open(cp.sha)"
                    >
                      <span class="text-highlighted truncate text-base font-medium">{{ cp.summary }}</span>
                      <span class="text-muted flex items-center gap-1.5 text-xs font-normal">
                        <UAvatar
                          :src="cp.avatarUrl"
                          :alt="cp.author"
                          size="3xs"
                          class="shrink-0"
                          data-testid="checkpoint-avatar"
                        />
                        <span><span data-testid="checkpoint-author">{{ cp.author }}</span> · {{ short(cp.sha) }} · {{ time(cp.ts) }}</span>
                      </span>
                    </button>
                    <UBadge v-if="cp.sha === currentSha" color="neutral" variant="soft" label="Current" class="shrink-0" />
                    <UTooltip v-if="deployOf(cp.sha)?.url" text="Open deployed site">
                      <UButton
                        color="neutral"
                        variant="ghost"
                        size="xs"
                        square
                        icon="i-hugeicons-globe-02"
                        :to="deployOf(cp.sha)!.url!"
                        target="_blank"
                        aria-label="Open deployed site"
                        class="relative z-10 shrink-0"
                        data-testid="checkpoint-row-deploy"
                        @click="openDeploy"
                      />
                    </UTooltip>
                    <UTooltip v-else-if="deployOf(cp.sha)?.state === 'building'" text="Deploying… the link appears once the site is live">
                      <span class="relative z-10 flex size-6 shrink-0 items-center justify-center" data-testid="checkpoint-row-deploy-building">
                        <UIcon name="i-hugeicons-loading-03" class="text-muted size-4 animate-spin" aria-label="Deploying" />
                      </span>
                    </UTooltip>
                    <UTooltip v-else-if="deployOf(cp.sha)?.state === 'failed'" text="This deploy failed">
                      <span class="relative z-10 flex size-6 shrink-0 items-center justify-center" data-testid="checkpoint-row-deploy-failed">
                        <UIcon name="i-hugeicons-alert-circle" class="text-error size-4" aria-label="Deploy failed" />
                      </span>
                    </UTooltip>
                    <UIcon name="i-hugeicons-arrow-right-01" class="text-muted size-4 shrink-0" aria-hidden="true" />
                  </div>
                </li>
              </template>
            </ol>
          </template>
        </div>
      </div>
      </div>
    </template>

    <UModal
      v-model:open="confirming"
      title="Restore this checkpoint?"
      :description="restoreDescription"
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
