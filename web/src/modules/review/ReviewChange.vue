<script setup lang="ts">
import { computed, defineAsyncComponent, ref, watch } from "vue";
import { useRouter } from "vue-router";
import type { SplitterItem, TabsItem } from "@nuxt/ui";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UBadge from "@nuxt/ui/components/Badge.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UEmpty from "@nuxt/ui/components/Empty.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import USplitter from "@nuxt/ui/components/Splitter.vue";
import UTabs from "@nuxt/ui/components/Tabs.vue";
import UTooltip from "@nuxt/ui/components/Tooltip.vue";
import { useDaemon } from "@/core/daemon";
import { BrowserFrame, isHttpUrl, type BrowserFrameSide } from "@/components/ui/browser-frame";
import { relativeTime } from "@/lib/relativeTime";
import { reviewPath, type ReviewPage } from "@/router";
import { RouterLink } from "vue-router";
import SendBackDialog from "./SendBackDialog.vue";
import { useReviewStore } from "./store";
import { useReviewPreview } from "./useReviewPreview";
import { useReviewConversation } from "./useReviewConversation";
import ReviewConversation from "./ReviewConversation.vue";
import { labelStyle } from "./labels";

// Lazy: the Pierre renderers bring Shiki along, which must stay out of the precached app shell.
const ChangedFilesTree = defineAsyncComponent(() => import("../checkpoints/ChangedFilesTree.vue"));
const DiffView = defineAsyncComponent(() => import("../checkpoints/DiffView.vue"));

const props = defineProps<{ projectId: string; number: number; page: ReviewPage }>();
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
const talk = useReviewConversation(
  () => props.projectId,
  () => props.number,
  () => change.value?.headSha,
);
/** Open / Draft / Merged / Closed, as GitHub labels the pull request. */
const STATES = {
  open: { label: "Open", color: "success", icon: "i-hugeicons-git-pull-request" },
  draft: { label: "Draft", color: "neutral", icon: "i-hugeicons-git-pull-request-draft" },
  merged: { label: "Merged", color: "secondary", icon: "i-hugeicons-git-merge" },
  closed: { label: "Closed", color: "error", icon: "i-hugeicons-git-pull-request-closed" },
} as const;
const prState = computed(() => {
  const c = change.value;
  if (!c) return STATES.open;
  if (c.merged) return STATES.merged;
  if (!c.open) return STATES.closed;
  return c.draft ? STATES.draft : STATES.open;
});
const filesCount = computed(() => store.diff?.files.length ?? null);
const PAGE_TAB =
  "text-muted hover:text-highlighted -mb-px flex items-center gap-1.5 border-b-2 border-transparent px-3 py-2 text-sm font-medium";
const PAGE_TAB_ACTIVE = "text-highlighted border-primary";
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

/** The change's deploy preview next to the live site; nothing is built on this machine. */
const { preview, error: previewError, loading: previewLoading, reload: reloadPreview } = useReviewPreview(
  () => props.projectId,
  () => props.number,
  () => change.value?.headSha,
);
const previewLive = computed(() => preview.value?.liveUrl ?? liveUrl.value);

type Compare = "change" | "live" | "side";
const COMPARE: TabsItem[] = [
  { label: "This change", value: "change" },
  { label: "Live site", value: "live" },
  { label: "Side by side", value: "side" },
];
/** `null` until the reviewer picks: side by side when the live site is there to compare with. */
const picked = ref<Compare | null>(null);
const compare = computed<Compare>(() => {
  if (picked.value) return picked.value;
  const coming = preview.value?.state === "ready" || preview.value?.state === "deploying";
  return coming && isHttpUrl(previewLive.value) ? "side" : "change";
});
watch(
  () => [props.projectId, props.number] as const,
  () => (picked.value = null),
);
const CHANGE_LABEL = "This change";
const LIVE_LABEL = "Live site";
const frames = computed<BrowserFrameSide[]>(() => {
  const mine = { key: "change", label: CHANGE_LABEL, url: preview.value?.url ?? null };
  const live = { key: "live", label: LIVE_LABEL, url: previewLive.value ?? null };
  if (compare.value === "change") return [mine];
  if (compare.value === "live") return [live];
  return [mine, live];
});
const checksUrl = computed(() => (change.value?.url ? `${change.value.url.replace(/\/+$/, "")}/checks` : null));

/** The rendered change fills the view by default; What changed is one click away. */
type View = "preview" | "changes";
const VIEWS: TabsItem[] = [
  { label: "Preview", value: "preview", icon: "i-hugeicons-computer-video" },
  { label: "What changed", value: "changes", icon: "i-hugeicons-file-validation" },
];
const view = ref<View>("preview");
watch(
  () => [props.projectId, props.number] as const,
  () => (view.value = "preview"),
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
        <p class="truncate text-base font-medium">
          <span class="text-highlighted" data-testid="review-title">{{ change.title }}</span>
          <span class="text-muted font-normal" data-testid="review-number"> #{{ change.number }}</span>
        </p>
        <div class="flex min-w-0 flex-wrap items-center gap-1.5">
          <UBadge
            :color="prState.color"
            variant="solid"
            :icon="prState.icon"
            :label="prState.label"
            data-testid="review-state"
          />
          <UBadge color="neutral" variant="soft" class="max-w-40 truncate" :label="change.author" />
          <UBadge color="neutral" variant="outline" :label="relativeTime(change.updatedAt) ?? ''" />
          <span
            v-for="label in change.labels ?? []"
            :key="label.name"
            class="rounded-full px-2 py-0.5 text-xs font-medium"
            :style="labelStyle(label.color)"
            data-testid="review-header-label"
          >{{ label.name }}</span>
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
      <nav class="border-default mx-3 mb-3 flex shrink-0 gap-1 border-b" aria-label="Change pages" data-testid="review-pages">
        <RouterLink
          :to="reviewPath(projectId, number)"
          :class="[PAGE_TAB, page === 'conversation' && PAGE_TAB_ACTIVE]"
          :aria-current="page === 'conversation' ? 'page' : undefined"
          data-testid="review-tab-conversation"
        >
          <UIcon name="i-hugeicons-message-multiple-01" class="size-4" />
          Conversation
          <UBadge v-if="talk.count.value !== null" color="neutral" variant="soft" size="sm" :label="String(talk.count.value)" />
        </RouterLink>
        <RouterLink
          :to="reviewPath(projectId, number, 'files')"
          :class="[PAGE_TAB, page === 'files' && PAGE_TAB_ACTIVE]"
          :aria-current="page === 'files' ? 'page' : undefined"
          data-testid="review-tab-files"
        >
          <UIcon name="i-hugeicons-file-validation" class="size-4" />
          Review
          <UBadge v-if="filesCount !== null" color="neutral" variant="soft" size="sm" :label="String(filesCount)" />
        </RouterLink>
      </nav>
      <ReviewConversation v-if="page === 'conversation'" :change="change" :talk="talk" :preview="preview" />
      <div
        v-else
        class="border-default bg-default mx-3 mb-3 flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border"
        data-testid="review-card"
      >
        <UTabs
          v-model="view"
          :items="VIEWS"
          :content="false"
          size="sm"
          variant="link"
          color="neutral"
          class="border-default shrink-0 border-b px-3"
          data-testid="review-view-tabs"
        />
        <div
          v-show="view === 'preview'"
          class="flex min-h-0 flex-1 flex-col"
          data-testid="review-preview"
        >
          <div class="border-default flex shrink-0 items-center border-b px-3 py-1.5">
            <UTabs
              :model-value="compare"
              :items="COMPARE"
              :content="false"
              size="xs"
              variant="pill"
              color="neutral"
              data-testid="review-compare"
              @update:model-value="picked = $event as Compare"
            />
          </div>
          <BrowserFrame :key="`${projectId}:${number}`" :frames="frames" class="min-h-0 flex-1">
            <template #placeholder="{ frame }">
              <UEmpty
                v-if="frame.key === 'live'"
                variant="naked"
                icon="i-hugeicons-globe-02"
                title="No live site found for this project."
                class="m-auto"
                data-testid="review-live-missing"
              />
              <UEmpty
                v-else-if="previewError"
                variant="naked"
                icon="i-hugeicons-alert-circle"
                title="The preview could not be loaded."
                :description="previewError"
                class="m-auto"
                role="alert"
                data-testid="review-preview-error"
              >
                <template #actions>
                  <UButton
                    color="neutral"
                    variant="outline"
                    size="sm"
                    label="Try again"
                    data-testid="review-preview-retry"
                    @click="reloadPreview()"
                  />
                </template>
              </UEmpty>
              <UEmpty
                v-else-if="preview?.state === 'deploying' || (!preview && previewLoading)"
                variant="naked"
                :title="preview ? 'Deploying preview…' : 'Loading preview…'"
                class="m-auto"
                data-testid="review-preview-deploying"
              >
                <template #leading>
                  <UIcon name="i-hugeicons-loading-03" class="text-muted size-5 animate-spin" role="status" />
                </template>
              </UEmpty>
              <UEmpty
                v-else-if="preview?.state === 'failed'"
                variant="naked"
                icon="i-hugeicons-alert-02"
                title="The preview deploy failed."
                class="m-auto"
                data-testid="review-preview-failed"
              >
                <template v-if="checksUrl" #actions>
                  <UButton
                    color="neutral"
                    variant="outline"
                    size="sm"
                    label="See the checks on GitHub"
                    trailing-icon="i-hugeicons-link-square-02"
                    :to="checksUrl"
                    target="_blank"
                    rel="noopener noreferrer"
                    data-testid="review-preview-checks"
                  />
                </template>
              </UEmpty>
              <UEmpty
                v-else-if="preview?.state === 'none'"
                variant="naked"
                icon="i-hugeicons-computer-video"
                title="This site doesn't publish previews for changes."
                description="See What changed."
                class="m-auto"
                data-testid="review-preview-none"
              >
                <template #actions>
                  <UButton
                    color="neutral"
                    variant="outline"
                    size="sm"
                    label="What changed"
                    data-testid="review-preview-to-changes"
                    @click="view = 'changes'"
                  />
                </template>
              </UEmpty>
            </template>
          </BrowserFrame>
        </div>
        <div v-show="view === 'changes'" class="flex min-h-0 flex-1 flex-col" data-testid="review-changes">
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
        </div>
      </div>
    </template>
  </div>
</template>
