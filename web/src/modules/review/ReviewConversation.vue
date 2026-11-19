<script setup lang="ts">
import { computed, defineAsyncComponent, ref } from "vue";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import UTextarea from "@nuxt/ui/components/Textarea.vue";
import type { ReviewDetail, ReviewPreview, ReviewReviewer, TimelineItem } from "@/core/types";
import { relativeTime } from "@/lib/relativeTime";
import { checkState, checkSummary } from "./checks";
import { labelStyle } from "./labels";
import type { useReviewConversation } from "./useReviewConversation";

// Lazy: the markdown renderer must stay out of the precached app shell.
const ReviewMarkdown = defineAsyncComponent(() => import("./ReviewMarkdown.vue"));

/** A change's Conversation page: description, timeline, comment box, and a sidebar of facts. */
const props = defineProps<{
  change: ReviewDetail;
  talk: ReturnType<typeof useReviewConversation>;
  preview: ReviewPreview | null;
}>();

const conversation = computed(() => props.talk.conversation.value);
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
  greenlit: { text: "greenlit these changes", icon: "i-hugeicons-checkmark-circle-02", class: "text-success" },
  sent_back: { text: "sent this back", icon: "i-hugeicons-alert-circle", class: "text-error" },
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

const draft = ref("");
async function submit() {
  if (await props.talk.comment(draft.value)) draft.value = "";
}
</script>

<template>
  <div class="min-h-0 flex-1 overflow-y-auto" data-testid="review-conversation">
    <div class="flex flex-col gap-6 p-4 lg:flex-row">
      <div class="flex min-w-0 flex-1 flex-col gap-4">
        <article class="border-default bg-default overflow-hidden rounded-lg border" data-testid="review-description">
          <header class="bg-muted border-default border-b px-3 py-2 text-sm">
            <span class="text-highlighted font-medium">{{ change.author }}</span>
            <span class="text-muted"> described this change</span>
          </header>
          <div class="p-3">
            <ReviewMarkdown v-if="change.description?.trim()" :content="change.description" />
            <p v-else class="text-muted text-sm italic">No description provided.</p>
          </div>
        </article>

        <p v-if="talk.loading.value && !conversation" class="text-muted flex items-center gap-2 text-sm">
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
          data-testid="review-conversation-error"
        >
          <template #actions>
            <UButton color="neutral" variant="outline" size="xs" label="Try again" @click="talk.reload()" />
          </template>
        </UAlert>

        <ol v-if="conversation" class="m-0 flex list-none flex-col gap-4 p-0" data-testid="review-timeline">
          <li v-for="(item, i) in conversation.items" :key="i" :data-kind="item.kind">
            <article
              v-if="item.kind === 'comment'"
              class="border-default bg-default overflow-hidden rounded-lg border"
              data-testid="review-timeline-comment"
            >
              <header class="bg-muted border-default flex items-center gap-1 border-b px-3 py-2 text-sm">
                <span class="text-highlighted font-medium">{{ item.author }}</span>
                <span class="text-muted">commented {{ relativeTime(item.at) }}</span>
              </header>
              <div class="p-3"><ReviewMarkdown :content="item.body" /></div>
            </article>

            <div v-else-if="item.kind === 'review'" data-testid="review-timeline-review" :data-verdict="item.verdict">
              <p class="flex items-center gap-2 text-sm">
                <UIcon :name="VERDICT[item.verdict].icon" :class="['size-4 shrink-0', VERDICT[item.verdict].class]" />
                <span>
                  <span class="text-highlighted font-medium">{{ item.author }}</span>
                  {{ VERDICT[item.verdict].text }}
                  <span class="text-muted">{{ relativeTime(item.at) }}</span>
                </span>
              </p>
              <div v-if="item.body" class="border-default bg-default mt-2 ml-6 rounded-lg border p-3">
                <ReviewMarkdown :content="item.body" />
              </div>
            </div>

            <div v-else-if="item.kind === 'push'" data-testid="review-timeline-push">
              <p class="flex items-center gap-2 text-sm">
                <UIcon name="i-hugeicons-git-commit" class="text-muted size-4 shrink-0" />
                <span>
                  <span class="text-highlighted font-medium">{{ pushers(item) }}</span>
                  pushed {{ commitsLabel(item.commits.length) }}
                  <span class="text-muted">{{ relativeTime(item.at) }}</span>
                </span>
              </p>
              <ul class="m-0 mt-1 ml-6 flex list-none flex-col gap-0.5 p-0 text-sm">
                <li v-for="c in item.commits" :key="c.sha" class="flex min-w-0 gap-2">
                  <code class="text-muted shrink-0 font-mono text-xs leading-5">{{ c.sha }}</code>
                  <span class="truncate">{{ c.message }}</span>
                </li>
              </ul>
            </div>

            <p v-else-if="item.kind === 'state'" class="flex items-center gap-2 text-sm" data-testid="review-timeline-state">
              <UIcon
                :name="item.state === 'merged' ? 'i-hugeicons-git-merge' : 'i-hugeicons-cancel-circle'"
                :class="['size-4 shrink-0', item.state === 'merged' ? 'text-violet-500' : 'text-error']"
              />
              <span>
                {{ item.state === "merged" ? "Merged" : "Closed" }}
                <span class="text-muted">{{ relativeTime(item.at) }}</span>
              </span>
            </p>
          </li>
        </ol>

        <form
          v-if="change.open"
          class="border-default bg-default flex flex-col gap-2 rounded-lg border p-3"
          data-testid="review-comment-form"
          @submit.prevent="submit"
        >
          <label for="review-comment" class="text-highlighted text-sm font-medium">Add a comment</label>
          <UTextarea
            id="review-comment"
            v-model="draft"
            :rows="4"
            autoresize
            placeholder="Add a comment"
            class="w-full"
            data-testid="review-comment-input"
          />
          <UAlert
            v-if="talk.postError.value"
            color="error"
            variant="subtle"
            icon="i-hugeicons-alert-circle"
            :description="talk.postError.value"
            role="alert"
            data-testid="review-comment-error"
          />
          <div class="flex items-center justify-between gap-2">
            <span class="text-muted text-xs">Markdown is supported.</span>
            <UButton
              type="submit"
              size="sm"
              label="Comment"
              :loading="talk.posting.value"
              :disabled="!draft.trim() || talk.posting.value"
              data-testid="review-comment-submit"
            />
          </div>
        </form>
      </div>

      <aside class="flex w-full shrink-0 flex-col gap-4 text-sm lg:w-64" data-testid="review-sidebar">
        <section class="border-default flex flex-col gap-1.5 border-b pb-3">
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

        <section class="border-default flex flex-col gap-1.5 border-b pb-3" data-testid="review-checks-summary">
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

        <section class="border-default flex flex-col gap-1.5 border-b pb-3">
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
  </div>
</template>
