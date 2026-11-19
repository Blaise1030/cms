<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, ref, watch } from "vue";
import { useRouter } from "vue-router";
import type { SplitterItem } from "@nuxt/ui";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UBadge from "@nuxt/ui/components/Badge.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UEmpty from "@nuxt/ui/components/Empty.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import USplitter from "@nuxt/ui/components/Splitter.vue";
import UTooltip from "@nuxt/ui/components/Tooltip.vue";
import { useDaemon } from "@/core/daemon";
import type { ReviewPreview } from "@/core/types";
import { errorMessage } from "@orso/utils";
import { relativeTime } from "@/lib/relativeTime";
import { reviewPath } from "@/router";
import SendBackDialog from "./SendBackDialog.vue";
import { useReviewStore } from "./store";

// Lazy: the Pierre renderers bring Shiki along, which must stay out of the precached app shell.
const ChangedFilesTree = defineAsyncComponent(() => import("../checkpoints/ChangedFilesTree.vue"));
const DiffView = defineAsyncComponent(() => import("../checkpoints/DiffView.vue"));

const props = defineProps<{ projectId: string; number: number }>();
const store = useReviewStore();
const daemon = useDaemon();
const router = useRouter();

const diffs = ref<HTMLElement | null>(null);
const liveUrl = ref<string | null>(null);
const sendingBack = ref(false);

watch(
  () => [props.projectId, props.number] as const,
  ([id, n]) => void store.open(id, n),
  { immediate: true },
);
// The live site link is a convenience; a failed lookup just hides it.
watch(
  () => props.projectId,
  async (id) => {
    try {
      const url = await daemon.api.liveSiteUrl(id);
      liveUrl.value = url && /^https?:\/\/./i.test(url) ? url : null;
    } catch {
      liveUrl.value = null;
    }
  },
  { immediate: true },
);

const change = computed(() => store.current);
const back = () => router.push(reviewPath(props.projectId));

function openSendBack() {
  store.decideError = null;
  sendingBack.value = true;
}
async function greenlight() {
  await store.decide("greenlight");
}
async function sendBack(comment: string) {
  // A moved change closes the dialog (and drops the comment) so the reviewer sees the reloaded
  // change before choosing again; other failures stay in the dialog with the comment.
  if ((await store.decide("send_back", comment)) || store.gone) sendingBack.value = false;
}

/** The rendered site of the change's head. A plain frame: the live-preview bridge belongs to the dev server. */
const POLL_MS = 2000;
const preview = ref<ReviewPreview | null>(null);
let pollTimer: ReturnType<typeof setTimeout> | null = null;
let previewRun = 0;

function stopPolling() {
  if (pollTimer) clearTimeout(pollTimer);
  pollTimer = null;
}

/** Applies an answer for run `run`; keeps polling while the site is still building. */
function settle(run: number, headSha: string, next: ReviewPreview) {
  if (run !== previewRun) return;
  preview.value = next;
  if (next.state === "building") pollTimer = setTimeout(() => void poll(run, headSha), POLL_MS);
}

async function poll(run: number, headSha: string) {
  try {
    const next = await daemon.api.reviewPreview(props.projectId, props.number, headSha);
    settle(run, headSha, next);
  } catch (e) {
    if (run === previewRun) preview.value = { state: "failed", message: errorMessage(e) };
  }
}

async function preparePreview() {
  stopPolling();
  const run = ++previewRun;
  const headSha = change.value?.headSha;
  if (!headSha) {
    preview.value = null;
    return;
  }
  preview.value = { state: "building" };
  try {
    settle(run, headSha, await daemon.api.prepareReviewPreview(props.projectId, props.number, headSha));
  } catch (e) {
    if (run === previewRun) preview.value = { state: "failed", message: errorMessage(e) };
  }
}

// Opening a change, or its head moving, starts the build for that head; leaving stops the polling.
watch(() => [props.projectId, props.number, change.value?.headSha] as const, () => void preparePreview(), {
  immediate: true,
});
onBeforeUnmount(() => {
  previewRun++;
  stopPolling();
});

const previewSrc = computed(() =>
  preview.value?.state === "ready" ? new URL(preview.value.url, daemon.config.value.origin).toString() : "",
);

const SPLIT: SplitterItem[] = [
  { id: "review-files-tree", slot: "tree", defaultSize: 20, minSize: 12, maxSize: 45 },
  { id: "review-diffs", slot: "diffs", defaultSize: 80, minSize: 40 },
];

/** Scroll a file's diff into view when it is picked in the changed-files tree. */
function reveal(path: string) {
  const section = [...(diffs.value?.querySelectorAll<HTMLElement>("section[data-path]") ?? [])].find(
    (el) => el.dataset.path === path,
  );
  section?.scrollIntoView({ behavior: "smooth", block: "start" });
}
</script>

<template>
  <div class="relative flex h-full min-h-0 flex-col" data-testid="review-change">
    <div class="flex min-h-10 shrink-0 items-center gap-3 px-2 py-1.5">
      <UTooltip text="Back to inbox">
        <UButton
          color="neutral"
          variant="outline"
          size="sm"
          icon="i-hugeicons-arrow-left-01"
          class="self-start"
          aria-label="Back to inbox"
          data-testid="review-back"
          @click="back()"
        />
      </UTooltip>
      <div v-if="change" class="min-w-0 flex-1 space-y-1">
        <p class="text-highlighted truncate text-base font-medium" data-testid="review-title">{{ change.title }}</p>
        <div class="flex min-w-0 flex-wrap items-center gap-1.5">
          <UBadge color="neutral" variant="soft" class="max-w-40 truncate" :label="change.author" />
          <UBadge color="neutral" variant="outline" :label="relativeTime(change.updatedAt) ?? ''" />
        </div>
      </div>
      <div v-else class="flex-1" />
      <div v-if="change" class="flex shrink-0 items-center gap-2">
        <p v-if="!change.canDecide && change.reason" class="text-muted max-w-64 text-xs" data-testid="review-reason">
          {{ change.reason }}
        </p>
        <UButton
          color="neutral"
          variant="outline"
          size="sm"
          label="Send back"
          :disabled="!change.canDecide || store.deciding"
          data-testid="review-send-back"
          @click="openSendBack()"
        />
        <UButton
          size="sm"
          label="Give greenlight"
          :loading="store.deciding"
          :disabled="!change.canDecide || store.deciding"
          data-testid="review-greenlight"
          @click="greenlight()"
        />
      </div>
      <UButton
        v-if="liveUrl"
        :href="liveUrl"
        target="_blank"
        rel="noopener noreferrer"
        color="neutral"
        variant="outline"
        size="sm"
        trailing-icon="i-hugeicons-link-square-02"
        label="View live site"
        class="shrink-0"
        data-testid="review-live-site"
      />
    </div>

    <UAlert
      v-if="store.decideError && !store.gone && !sendingBack"
      color="warning"
      variant="subtle"
      icon="i-hugeicons-alert-circle"
      :description="store.decideError"
      role="alert"
      class="mx-3 mb-2 w-auto shrink-0"
      data-testid="review-decide-error"
    />
    <SendBackDialog v-model:open="sendingBack" :busy="store.deciding" :error="store.decideError" @submit="sendBack" />
    <p v-if="store.opening" class="text-muted flex items-center gap-2 p-3 text-sm">
      <UIcon name="i-hugeicons-loading-03" class="size-4 animate-spin" />
      Loading change…
    </p>
    <UEmpty
      v-else-if="store.gone"
      variant="naked"
      icon="i-hugeicons-task-done-01"
      title="This change is no longer waiting"
    >
      <template #actions>
        <UButton color="neutral" variant="outline" label="Back to inbox" data-testid="review-gone-back" @click="back()" />
      </template>
    </UEmpty>
    <div v-else-if="store.openError" class="p-3">
      <UAlert color="error" variant="subtle" icon="i-hugeicons-alert-circle" :description="store.openError" role="alert" />
    </div>
    <template v-else-if="change">
      <p
        v-if="change.description"
        class="text-default max-h-32 shrink-0 overflow-y-auto px-3 pb-2 text-sm whitespace-pre-wrap"
        data-testid="review-description"
      >
        {{ change.description }}
      </p>
      <h2 class="text-muted m-0 shrink-0 px-3 pb-1 text-xs font-medium">Preview</h2>
      <div class="bg-muted mx-3 mb-2 h-64 shrink-0 overflow-hidden rounded-md" data-testid="review-preview">
        <p
          v-if="preview?.state === 'building'"
          class="text-muted flex h-full items-center justify-center gap-2 text-sm"
          data-testid="review-preview-building"
        >
          <UIcon name="i-hugeicons-loading-03" class="size-4 animate-spin" />
          Preparing the preview…
        </p>
        <iframe
          v-else-if="preview?.state === 'ready'"
          :key="previewSrc"
          :src="previewSrc"
          title="Preview of the change"
          class="bg-default h-full w-full border-0"
          sandbox="allow-same-origin"
          referrerpolicy="no-referrer"
          data-testid="review-preview-frame"
        />
        <div
          v-else-if="preview?.state === 'failed'"
          class="flex h-full flex-col items-start gap-2 overflow-hidden p-3"
          data-testid="review-preview-failed"
        >
          <p class="text-default shrink-0 text-sm">The preview could not be prepared.</p>
          <pre class="text-muted min-h-0 w-full flex-1 overflow-auto font-mono text-xs whitespace-pre-wrap">{{ preview.message }}</pre>
          <UButton
            color="neutral"
            variant="outline"
            size="sm"
            label="Try again"
            class="shrink-0"
            data-testid="review-preview-retry"
            @click="preparePreview()"
          />
        </div>
      </div>
      <h2 class="text-muted m-0 shrink-0 px-3 pb-1 text-xs font-medium">What changed</h2>
      <UEmpty
        v-if="store.diff && store.diff.files.length === 0"
        variant="naked"
        icon="i-hugeicons-file-validation"
        title="No file changes in this change."
      />
      <USplitter
        v-else-if="store.diff"
        :items="SPLIT"
        auto-save-id="review-files-sidebar"
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
  </div>
</template>
