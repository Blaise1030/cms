<script setup lang="ts">
import { computed, defineAsyncComponent, nextTick, provide, ref, watch } from "vue";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UBadge from "@nuxt/ui/components/Badge.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UChatMessage from "@nuxt/ui/components/ChatMessage.vue";
import UChatMessages from "@nuxt/ui/components/ChatMessages.vue";
import UChatPrompt from "@nuxt/ui/components/ChatPrompt.vue";
import UChatPromptSubmit from "@nuxt/ui/components/ChatPromptSubmit.vue";
import UCollapsible from "@nuxt/ui/components/Collapsible.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import UTooltip from "@nuxt/ui/components/Tooltip.vue";
import type { UIMessage } from "ai";
import type { ButtonProps } from "@nuxt/ui";
import type { ReviewDetail, ReviewPreview, ReviewReviewer, TimelineItem } from "@/core/types";
import { checkState, checkSummary } from "./checks";
import { useDaemon } from "@/core/daemon";
import { ASSET_IMAGES, repoOfPullUrl, rewriteAssetImages } from "./assetImages";
import { buildFeed, clockTime, exactTime } from "./feed";
import { labelStyle } from "./labels";
import type { useConversation } from "./queries";

// Lazy: the markdown renderer must stay out of the precached app shell.
const ReviewMarkdown = defineAsyncComponent(() => import("./ReviewMarkdown.vue"));

/**
 * A change's Conversation page, laid out like a chat: the description pinned on top, the feed
 * oldest first (opening on the latest), a composer pinned at the bottom, and a sidebar of facts.
 */
const props = defineProps<{
  projectId: string;
  change: ReviewDetail;
  talk: ReturnType<typeof useConversation>;
  preview: ReviewPreview | null;
}>();

const conversation = computed(() => props.talk.conversation.value);

// Images orso stored on the repo's assets branch load through the daemon, private repos too.
const daemon = useDaemon();
provide(ASSET_IMAGES, (markdown: string) => {
  const repo = repoOfPullUrl(props.change.url);
  return repo
    ? rewriteAssetImages(markdown, repo, (sha, path) => daemon.api.reviewAssetUrl(props.projectId, sha, path))
    : markdown;
});
const rows = computed(() => buildFeed(conversation.value?.items ?? []));
const checks = computed(() => conversation.value?.checks ?? []);
const summary = computed(() => checkSummary(checks.value));
const checksOpen = ref(false);

const SUMMARY_ICON = {
  passing: { name: "i-hugeicons-checkmark-circle-02", class: "text-success" },
  failing: { name: "i-hugeicons-cancel-circle", class: "text-error" },
  pending: { name: "i-hugeicons-clock-01", class: "text-warning" },
  none: { name: "i-hugeicons-minus-sign-circle", class: "text-muted" },
} as const;

const VERDICT = {
  greenlit: { text: "greenlit this change", icon: "i-hugeicons-checkmark-circle-02", class: "text-success" },
  sent_back: { text: "sent this back", icon: "i-hugeicons-arrow-turn-backward", class: "text-error" },
  commented: { text: "reviewed", icon: "i-hugeicons-message-01", class: "text-muted" },
} as const;

const REVIEWER: Record<ReviewReviewer["state"], { label: string; class: string }> = {
  requested: { label: "Awaiting review", class: "text-muted" },
  commented: { label: "Commented", class: "text-muted" },
  greenlit: { label: "Greenlit", class: "text-success" },
  sent_back: { label: "Sent back", class: "text-error" },
  dismissed: { label: "Dismissed", class: "text-muted" },
};

const pushers = (item: Extract<TimelineItem, { kind: "push" }>) =>
  [...new Set(item.commits.map((c) => c.author).filter(Boolean))].join(", ") || "Someone";
const commitsLabel = (n: number) => `${n} commit${n === 1 ? "" : "s"}`;
/** orso's checkpoint commits read "checkpoint: …": shown as a badge plus the subject. */
function commitSubject(message: string): { checkpoint: boolean; text: string } {
  const m = message.match(/^checkpoint:\s*/i);
  return m ? { checkpoint: true, text: message.slice(m[0].length) } : { checkpoint: false, text: message };
}

/** A description longer than this folds to one line until opened. */
const LONG_DESCRIPTION = 280;
const description = computed(() => props.change.description?.trim() ?? "");
const longDescription = computed(
  () => description.value.length > LONG_DESCRIPTION || description.value.split("\n").length > 4,
);
/** The description's first line without markdown marks, for the folded card. */
const excerpt = computed(() =>
  (description.value.split("\n").find((l) => l.trim()) ?? "").replace(/[#>*_`~[\]]/g, "").trim(),
);
const descriptionOpen = ref(false);

/**
 * The feed is the assistant chat's: `UChatMessages` opens on the latest entry and offers a
 * jump to the bottom whenever you've scrolled away. Outside streaming it doesn't follow new
 * entries on its own, so (like `ChatTranscript`) a new entry keeps the view at the bottom when
 * it already was, and sending always lands there.
 */
const viewport = ref<HTMLElement | null>(null);
/** Same distance `UChatMessages` treats as "at the bottom". */
const BOTTOM_THRESHOLD = 100;
/** `UChatMessages` keys its scroll state on these; each row is one entry. */
const feedMessages = computed<UIMessage[]>(() =>
  rows.value.map((r) => ({ id: r.key, role: r.type === "message" ? "assistant" : "system", parts: [] })),
);
/** The hover toolbar's "Open on GitHub" (its label is the tooltip and the aria-label). */
function openOnGitHub(url: string | undefined): Omit<ButtonProps, "onClick">[] | undefined {
  if (!url) return undefined;
  const open = { icon: "i-hugeicons-link-square-02", label: "Open on GitHub", to: url, target: "_blank" };
  return [{ ...open, rel: "noopener noreferrer", "data-testid": "review-message-open" } as Omit<ButtonProps, "onClick">];
}

let sent = false;
watch(
  () => rows.value.length,
  (count, before) => {
    const el = viewport.value;
    if (!el || !before || count <= before) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < BOTTOM_THRESHOLD;
    const follow = atBottom || sent;
    sent = false;
    if (follow) void nextTick(() => (el.scrollTop = el.scrollHeight));
  },
  { flush: "pre" },
);

const draft = ref("");
/** Sends at once: the composer clears and the message shows as "Sending…" until GitHub has it. */
function submit() {
  if (!draft.value.trim()) return;
  sent = true;
  if (props.talk.comment(draft.value)) draft.value = "";
  else sent = false;
}
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col lg:flex-row" data-testid="review-conversation">
    <section class="relative flex min-h-0 min-w-0 flex-1 flex-col" aria-label="Conversation">
      <article
        class="border-default bg-default mx-4 mt-4 shrink-0 overflow-hidden rounded-lg border"
        data-testid="review-description"
      >
        <UCollapsible v-if="longDescription" v-model:open="descriptionOpen" :unmount-on-hide="false">
          <button
            type="button"
            class="hover:bg-elevated/50 flex w-full min-w-0 items-center gap-2 px-3 py-2 text-left text-sm"
            data-testid="review-description-toggle"
          >
            <span class="min-w-0 flex-1 truncate">
              <span class="text-highlighted font-medium">{{ change.author }}</span>
              <span class="text-muted"> described this change</span>
              <span v-if="!descriptionOpen" class="text-muted"> · {{ excerpt }}</span>
            </span>
            <UIcon
              :name="descriptionOpen ? 'i-hugeicons-arrow-up-01' : 'i-hugeicons-arrow-down-01'"
              class="text-muted size-4 shrink-0"
            />
          </button>
          <template #content>
            <div class="border-default max-h-64 overflow-y-auto border-t p-3">
              <ReviewMarkdown :content="description" />
            </div>
          </template>
        </UCollapsible>
        <template v-else>
          <header class="px-3 pt-2 text-sm">
            <span class="text-highlighted font-medium">{{ change.author }}</span>
            <span class="text-muted"> described this change</span>
          </header>
          <div class="px-3 pt-1 pb-2">
            <ReviewMarkdown v-if="description" :content="description" />
            <p v-else class="text-muted text-sm italic">No description provided.</p>
          </div>
        </template>
      </article>

      <div
        ref="viewport"
        class="scroll-fade-y relative min-h-0 flex-1 overflow-y-auto px-2 py-3"
        data-testid="review-feed"
      >
        <p v-if="talk.loading.value && !conversation" class="text-muted flex items-center gap-2 px-2 text-sm">
          <UIcon name="i-hugeicons-loading-03" class="size-4 animate-spin" />
          Loading conversation…
        </p>
        <UAlert
          v-else-if="talk.error.value && !conversation"
          color="error"
          variant="subtle"
          icon="i-hugeicons-alert-circle"
          :description="talk.error.value"
          role="alert"
          class="mx-2"
          data-testid="review-conversation-error"
        >
          <template #actions>
            <UButton color="neutral" variant="outline" size="xs" label="Try again" @click="talk.reload()" />
          </template>
        </UAlert>
        <p v-else-if="conversation && rows.length === 0" class="text-muted py-6 text-center text-sm">
          No messages yet.
        </p>

        <UChatMessages
          v-if="conversation && rows.length"
          :messages="feedMessages"
          status="ready"
          compact
          data-testid="review-timeline"
          :ui="{
            root: 'gap-0.5',
            // The jump button sticks to the bottom edge of the scroll container.
            viewport: 'pointer-events-none sticky bottom-0 z-10 h-0',
            autoScroll: 'pointer-events-auto bottom-2',
          }"
        >
          <template #default>
            <template v-for="row in rows" :key="row.key">
              <div
                v-if="row.type === 'day'"
                role="separator"
                :aria-label="row.label"
                data-kind="day"
                class="flex items-center justify-center px-1 py-3"
                data-testid="review-day"
              >
                <UBadge color="neutral" variant="soft" size="sm" :label="row.label" />
              </div>

              <UChatMessage
                v-else-if="row.type === 'message'"
                :id="row.key"
                role="assistant"
                :parts="[]"
                side="left"
                variant="naked"
                compact
                :avatar="{ src: row.avatarUrl, alt: row.author }"
                :actions="openOnGitHub(row.url)"
                data-kind="comment"
                :data-grouped="row.grouped || undefined"
                data-testid="review-timeline-comment"
                :data-pending="row.pending"
                :class="[
                  'hover:bg-elevated/50 rounded-md px-1',
                  row.grouped ? 'py-0.5' : 'mt-1.5 py-1',
                  row.pending === 'sending' && 'opacity-60',
                ]"
                :ui="{
                  container: 'gap-2.5 pb-0',
                  leading: 'min-h-0 pt-0.5',
                  leadingAvatarSize: 'sm',
                  content: 'flex flex-col gap-0.5 space-y-0 text-sm',
                  actions: 'border-default bg-default top-0 right-1 bottom-auto -translate-y-1/2 rounded-md border shadow-xs',
                }"
              >
                <!-- Follow-ups keep the avatar's width, showing their time on hover instead. -->
                <template v-if="row.grouped" #leading>
                  <UTooltip :text="exactTime(row.at)">
                    <time
                      :datetime="row.at"
                      class="text-dimmed invisible block w-7 text-right text-[10px] leading-5 group-hover/message:visible"
                    >{{ clockTime(row.at) }}</time>
                  </UTooltip>
                </template>
                <template #content>
                  <p v-if="!row.grouped" class="flex items-baseline gap-2" data-testid="review-message-header">
                    <span class="text-highlighted font-semibold">{{ row.author }}</span>
                    <span v-if="row.pending === 'sending'" class="text-muted text-xs" data-testid="review-message-sending">Sending…</span>
                    <UTooltip v-else-if="!row.pending" :text="exactTime(row.at)">
                      <time :datetime="row.at" class="text-muted text-xs">{{ clockTime(row.at) }}</time>
                    </UTooltip>
                  </p>
                  <ReviewMarkdown :content="row.body" />
                  <p v-if="row.grouped && row.pending === 'sending'" class="text-muted text-xs" data-testid="review-message-sending">
                    Sending…
                  </p>
                  <p v-if="row.pending === 'failed'" class="text-error flex items-center gap-1 text-xs" data-testid="review-message-failed">
                    <UIcon name="i-hugeicons-alert-circle" class="size-3.5 shrink-0" />
                    Couldn't send.
                    <UButton
                      color="error"
                      variant="link"
                      size="xs"
                      class="p-0"
                      label="Retry"
                      data-testid="review-message-retry"
                      @click="talk.retry(row.key)"
                    />
                    <span aria-hidden="true">·</span>
                    <UButton
                      color="error"
                      variant="link"
                      size="xs"
                      class="p-0"
                      label="Delete"
                      data-testid="review-message-delete"
                      @click="talk.discard(row.key)"
                    />
                  </p>
                </template>
              </UChatMessage>

            <div v-else :data-kind="row.item.kind" class="px-1 py-1.5" data-testid="review-feed-row">
              <div
                v-if="row.item.kind === 'review'"
                data-testid="review-timeline-review"
                :data-verdict="row.item.verdict"
              >
                <p class="text-muted flex items-center gap-2 text-sm">
                  <UIcon :name="VERDICT[row.item.verdict].icon" :class="['size-4 shrink-0', VERDICT[row.item.verdict].class]" />
                  <span>
                    <span class="text-highlighted font-medium">{{ row.item.author }}</span>
                    {{ VERDICT[row.item.verdict].text }}
                  </span>
                  <UTooltip :text="exactTime(row.item.at)">
                    <time :datetime="row.item.at" class="text-dimmed text-xs">{{ clockTime(row.item.at) }}</time>
                  </UTooltip>
                </p>
                <blockquote
                  v-if="row.item.body"
                  class="border-accented mt-1 ml-6 border-l-2 pl-3 text-sm"
                  data-testid="review-verdict-comment"
                >
                  <ReviewMarkdown :content="row.item.body" />
                </blockquote>
              </div>

              <div v-else-if="row.item.kind === 'push'" data-testid="review-timeline-push">
                <UCollapsible>
                  <template #default="{ open }">
                    <UButton
                      color="neutral"
                      variant="ghost"
                      size="xs"
                      class="text-muted -mx-1.5 max-w-full font-normal"
                      data-testid="review-push-toggle"
                    >
                      <UIcon name="i-hugeicons-git-commit" class="size-4 shrink-0" />
                      <span class="truncate text-sm">
                        <span class="text-highlighted font-medium">{{ pushers(row.item) }}</span>
                        pushed {{ commitsLabel(row.item.commits.length) }}
                      </span>
                      <UIcon :name="open ? 'i-hugeicons-arrow-down-01' : 'i-hugeicons-arrow-right-01'" class="size-3.5 shrink-0" />
                      <time :datetime="row.item.at" class="text-dimmed text-xs">{{ clockTime(row.item.at) }}</time>
                    </UButton>
                  </template>
                  <template #content>
                    <ul class="m-0 mt-1 ml-6 flex list-none flex-col gap-0.5 p-0 text-sm">
                      <li
                        v-for="c in row.item.commits"
                        :key="c.sha"
                        class="flex min-w-0 items-center gap-2"
                        data-testid="review-push-commit"
                      >
                        <code class="text-muted shrink-0 font-mono text-xs leading-5">{{ c.sha }}</code>
                        <UBadge
                          v-if="commitSubject(c.message).checkpoint"
                          color="neutral"
                          variant="soft"
                          size="sm"
                          label="checkpoint"
                          class="text-muted shrink-0"
                          data-testid="review-commit-checkpoint"
                        />
                        <span class="truncate" data-testid="review-commit-subject">{{ commitSubject(c.message).text }}</span>
                      </li>
                    </ul>
                  </template>
                </UCollapsible>
              </div>

              <p v-else-if="row.item.kind === 'state'" class="text-muted flex items-center gap-2 text-sm" data-testid="review-timeline-state">
                <UIcon
                  :name="row.item.state === 'merged' ? 'i-hugeicons-git-merge' : 'i-hugeicons-cancel-circle'"
                  :class="['size-4 shrink-0', row.item.state === 'merged' ? 'text-violet-500' : 'text-error']"
                />
                <span class="text-highlighted">{{ row.item.state === "merged" ? "Merged" : "Closed" }}</span>
                <time :datetime="row.item.at" class="text-dimmed text-xs">{{ clockTime(row.item.at) }}</time>
              </p>
            </div>
            </template>
          </template>
        </UChatMessages>
      </div>

      <div v-if="change.open" class="shrink-0 px-4 pt-1 pb-4" data-testid="review-comment-form">
        <!-- ⌘/Ctrl+Enter sends; Enter alone is a new line. -->
        <UChatPrompt
          v-model="draft"
          variant="outline"
          placeholder="Message…"
          :submit-on-enter="false"
          :autofocus="false"
          :maxrows="8"
          aria-label="Message"
          data-testid="review-comment-input"
          :ui="{ base: 'max-h-48 min-h-10 px-0 py-1' }"
          @submit="submit"
        >
          <template #footer>
            <span class="text-dimmed text-xs">Markdown · ⌘/Ctrl+Enter to send</span>
            <UChatPromptSubmit
              status="ready"
              size="sm"
              label="Send"
              :disabled="!draft.trim()"
              data-testid="review-comment-submit"
            />
          </template>
        </UChatPrompt>
      </div>
    </section>

      <aside
        class="border-default flex w-full shrink-0 flex-col gap-4 overflow-y-auto p-4 text-sm max-lg:max-h-64 max-lg:border-t lg:w-64"
        data-testid="review-sidebar"
      >
        <section class="flex flex-col gap-1.5">
          <h3 class="text-muted text-xs font-medium">Labels</h3>
          <div v-if="change.labels?.length" class="flex flex-wrap gap-1">
            <span
              v-for="label in change.labels"
              :key="label.name"
              class="rounded-full px-2 py-0.5 text-xs font-medium"
              :style="labelStyle(label.color)"
              data-testid="review-label"
            >{{ label.name }}</span>
          </div>
          <p v-else class="text-muted">None</p>
        </section>

        <section class="flex flex-col gap-1.5" data-testid="review-checks-summary">
          <h3 class="text-muted text-xs font-medium">Checks</h3>
          <p v-if="!checks.length" class="text-muted">None</p>
          <template v-else>
            <UButton
              color="neutral"
              variant="ghost"
              size="xs"
              class="-mx-1.5 w-fit"
              :trailing-icon="checksOpen ? 'i-hugeicons-arrow-up-01' : 'i-hugeicons-arrow-down-01'"
              :aria-expanded="checksOpen"
              data-testid="review-checks-toggle"
              @click="checksOpen = !checksOpen"
            >
              <UIcon :name="SUMMARY_ICON[summary.state].name" :class="['size-4', SUMMARY_ICON[summary.state].class]" />
              <span class="tabular-nums" data-testid="review-checks-count">{{ summary.passed }}/{{ summary.total }}</span>
            </UButton>
            <ul v-if="checksOpen" class="m-0 flex list-none flex-col gap-1 p-0" data-testid="review-checks-list">
              <li v-for="c in checks" :key="c.name + c.url" class="flex min-w-0 items-center gap-1.5">
                <UIcon
                  :name="SUMMARY_ICON[checkState(c)].name"
                  :class="['size-3.5 shrink-0', SUMMARY_ICON[checkState(c)].class]"
                />
                <a
                  v-if="c.url"
                  :href="c.url"
                  target="_blank"
                  rel="noopener noreferrer"
                  class="text-default min-w-0 flex-1 truncate hover:underline"
                >{{ c.name }}</a>
                <span v-else class="min-w-0 flex-1 truncate">{{ c.name }}</span>
                <span class="text-muted shrink-0 text-xs">{{ c.conclusion ?? c.status.replace("_", " ") }}</span>
              </li>
            </ul>
          </template>
        </section>

        <section class="flex flex-col gap-1.5">
          <h3 class="text-muted text-xs font-medium">Deploy preview</h3>
          <a
            v-if="preview?.url"
            :href="preview.url"
            target="_blank"
            rel="noopener noreferrer"
            class="text-primary flex items-center gap-1 truncate hover:underline"
            data-testid="review-deploy-link"
          >
            Open deploy preview
            <UIcon name="i-hugeicons-link-square-02" class="size-3.5 shrink-0" />
          </a>
          <p v-else-if="preview?.state === 'deploying'" class="text-muted">Deploying…</p>
          <p v-else-if="preview?.state === 'failed'" class="text-error">Deploy failed</p>
          <p v-else class="text-muted">None</p>
        </section>

        <section class="flex flex-col gap-1.5">
          <h3 class="text-muted text-xs font-medium">Reviewers</h3>
          <ul v-if="conversation?.reviewers.length" class="m-0 flex list-none flex-col gap-1 p-0">
            <li
              v-for="r in conversation.reviewers"
              :key="r.login"
              class="flex items-center justify-between gap-2"
              data-testid="review-reviewer"
            >
              <span class="truncate">{{ r.login }}</span>
              <span :class="['shrink-0 text-xs', REVIEWER[r.state].class]">{{ REVIEWER[r.state].label }}</span>
            </li>
          </ul>
          <p v-else class="text-muted">No reviews yet</p>
        </section>
      </aside>
  </div>
</template>
