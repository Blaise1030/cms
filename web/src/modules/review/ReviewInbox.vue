<script setup lang="ts">
import { computed, reactive, ref, toRefs } from "vue";
import { RouterLink, useRoute, useRouter } from "vue-router";
import type { DropdownMenuItem } from "@nuxt/ui";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UBadge from "@nuxt/ui/components/Badge.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UDropdownMenu from "@nuxt/ui/components/DropdownMenu.vue";
import UEmpty from "@nuxt/ui/components/Empty.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import UInput from "@nuxt/ui/components/Input.vue";
import type { ChecksState, MyReviewState, ReviewItem } from "@/core/types";
import { relativeTime } from "@/lib/relativeTime";
import { reviewPath } from "@/router";
import { applyQuery, DEFAULT_QUERY, parseQuery, setQualifier, SORT_KEY, type InboxSort, type ReviewFilter } from "./inboxQuery";
import { labelStyle } from "./labels";
import { useClosedInbox, useInbox } from "./queries";

const props = defineProps<{ projectId: string }>();
const inbox = useInbox(() => props.projectId);
const closed = useClosedInbox(() => props.projectId);
/** One object for the template: open changes (shared with the rail) and closed ones. */
const store = reactive({ ...toRefs(inbox), ...toRefs(closed) });
const route = useRoute();
const router = useRouter();

/** The search lives in `?q=`, so it survives a reload and can be shared. */
const q = computed(() => (typeof route.query.q === "string" ? route.query.q : DEFAULT_QUERY));
const setQ = (value: string) => void router.replace({ query: { ...route.query, q: value } });
const query = computed(() => parseQuery(q.value));
const setQualifierTo = (key: string, value: string | null) => setQ(setQualifier(q.value, key, value));
/** Picking the active option again clears it, like GitHub's menus. */
const toggle = (key: string, current: string | null, value: string) =>
  setQualifierTo(key, current?.toLowerCase() === value.toLowerCase() ? null : value);

const openRows = computed(() => applyQuery(store.items, query.value));
const closedRows = computed(() => applyQuery(store.closedItems ?? [], query.value));
const rows = computed(() => (query.value.state === "closed" ? closedRows.value : openRows.value));
const source = computed(() => (query.value.state === "closed" ? (store.closedItems ?? []) : store.items));
const loading = computed(() =>
  query.value.state === "closed" ? store.closedLoading && store.closedItems === null : store.loading && store.items.length === 0,
);
const error = computed(() =>
  query.value.state === "closed"
    ? store.closedItems === null
      ? store.closedError
      : null
    : store.items.length === 0
      ? store.inboxError
      : null,
);
const closedCount = computed(() =>
  store.closedItems === null ? null : `${closedRows.value.length}${store.closedMore ? "+" : ""}`,
);
/** Anything beyond `is:`/`sort:` narrows the list: an empty result means "no match", not "nothing". */
const searching = computed(
  () => !!(query.value.text || query.value.author || query.value.label || query.value.review),
);

const authors = computed(() => [...new Set(source.value.map((i) => i.author))].sort((a, b) => a.localeCompare(b)));
const labels = computed(() => {
  const seen = new Map<string, string>();
  for (const item of source.value) for (const l of item.labels ?? []) if (!seen.has(l.name)) seen.set(l.name, l.color);
  return [...seen].map(([name, color]) => ({ name, color })).sort((a, b) => a.name.localeCompare(b.name));
});

const authorItems = computed<DropdownMenuItem[]>(() =>
  authors.value.map((a) => ({
    type: "checkbox",
    label: a,
    checked: query.value.author?.toLowerCase() === a.toLowerCase(),
    testid: `review-filter-author-${a}`,
    onUpdateChecked: () => toggle("author", query.value.author, a),
  })),
);
const labelItems = computed<DropdownMenuItem[]>(() =>
  labels.value.map((l) => ({
    type: "checkbox",
    label: l.name,
    color: l.color,
    checked: query.value.label?.toLowerCase() === l.name.toLowerCase(),
    testid: `review-filter-label-${l.name}`,
    onUpdateChecked: () => toggle("label", query.value.label, l.name),
  })),
);
const REVIEW_FILTERS: { id: ReviewFilter; label: string }[] = [
  { id: "new", label: "New" },
  { id: "greenlit", label: "You greenlit" },
  { id: "sent_back", label: "You sent back" },
  { id: "yours", label: "Yours" },
];
const reviewItems = computed<DropdownMenuItem[]>(() =>
  REVIEW_FILTERS.map((r) => ({
    type: "checkbox",
    label: r.label,
    checked: query.value.review === r.id,
    testid: `review-filter-review-${r.id}`,
    onUpdateChecked: () => toggle("review", query.value.review, r.id),
  })),
);
const SORTS: { id: InboxSort; label: string }[] = [
  { id: "newest", label: "Newest" },
  { id: "oldest", label: "Oldest" },
  { id: "updated", label: "Recently updated" },
];
const sortItems = computed<DropdownMenuItem[]>(() =>
  SORTS.map((s) => ({
    type: "checkbox",
    label: s.label,
    checked: query.value.sort === s.id,
    testid: `review-sort-${s.id}`,
    // Newest is the default, so it leaves no qualifier behind.
    onUpdateChecked: () => setQualifierTo("sort", s.id === "newest" ? null : SORT_KEY[s.id]),
  })),
);
const sortLabel = computed(() => SORTS.find((s) => s.id === query.value.sort)!.label);
const MENUS = computed(() => [
  { id: "author", label: "Author", items: authorItems.value, active: !!query.value.author },
  { id: "label", label: "Label", items: labelItems.value, active: !!query.value.label },
  { id: "reviews", label: "Reviews", items: reviewItems.value, active: !!query.value.review },
]);

const STATE: Record<MyReviewState, { label: string; color: "primary" | "warning" | "success" }> = {
  new: { label: "New", color: "primary" },
  sent_back: { label: "You sent back", color: "warning" },
  greenlit: { label: "You greenlit", color: "success" },
};
const CHECKS: Record<Exclude<ChecksState, "none">, { icon: string; class: string; label: string }> = {
  pending: { icon: "i-hugeicons-clock-01", class: "text-warning", label: "Checks running" },
  passing: { icon: "i-hugeicons-checkmark-circle-02", class: "text-success", label: "Checks passing" },
  failing: { icon: "i-hugeicons-cancel-circle", class: "text-error", label: "Checks failing" },
};
/** GitHub's pull request icon per state. */
function prIcon(item: ReviewItem) {
  if (item.state === "merged") return { name: "i-hugeicons-git-merge", class: "text-violet-500", label: "Merged" };
  if (item.state === "closed") return { name: "i-hugeicons-git-pull-request-closed", class: "text-error", label: "Closed" };
  if (item.draft) return { name: "i-hugeicons-git-pull-request-draft", class: "text-muted", label: "Draft" };
  return { name: "i-hugeicons-git-pull-request", class: "text-success", label: "Open" };
}

const draft = ref<string | null>(null);
const open = (number: number) => router.push(reviewPath(props.projectId, number));
</script>

<template>
  <div class="min-h-0 flex-1 overflow-y-auto" data-testid="review-inbox">
    <div class="flex min-h-full flex-col gap-3 p-3">
      <form class="flex shrink-0" role="search" @submit.prevent="draft !== null && setQ(draft)">
        <UInput
          :model-value="draft ?? q"
          icon="i-hugeicons-search-01"
          size="md"
          class="w-full"
          placeholder="Search changes"
          aria-label="Search changes"
          spellcheck="false"
          autocomplete="off"
          data-testid="review-search"
          @update:model-value="setQ(String($event))"
        >
          <template v-if="q" #trailing>
            <UButton
              color="neutral"
              variant="link"
              size="sm"
              icon="i-hugeicons-cancel-circle"
              aria-label="Clear search"
              data-testid="review-search-clear"
              @click="setQ('')"
            />
          </template>
        </UInput>
      </form>

      <div
        class="border-default bg-default flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border"
        data-testid="review-list-card"
      >
        <div class="bg-muted border-default flex shrink-0 flex-wrap items-center gap-1 border-b px-2 py-1.5">
          <UButton
            color="neutral"
            :variant="query.state === 'open' ? 'soft' : 'ghost'"
            size="sm"
            icon="i-hugeicons-git-pull-request"
            :aria-pressed="query.state === 'open'"
            data-testid="review-show-open"
            @click="setQualifierTo('is', 'open')"
          >
            Open <span class="tabular-nums" data-testid="review-open-count">{{ openRows.length }}</span>
          </UButton>
          <UButton
            color="neutral"
            :variant="query.state === 'closed' ? 'soft' : 'ghost'"
            size="sm"
            icon="i-hugeicons-checkmark-circle-02"
            :aria-pressed="query.state === 'closed'"
            data-testid="review-show-closed"
            @click="setQualifierTo('is', 'closed')"
          >
            Closed <span v-if="closedCount !== null" class="tabular-nums" data-testid="review-closed-count">{{ closedCount }}</span>
          </UButton>
          <div class="ml-auto flex flex-wrap items-center gap-0.5">
            <UDropdownMenu
              v-for="menu in MENUS"
              :key="menu.id"
              :items="menu.items"
              :content="{ align: 'end', side: 'bottom' }"
            >
              <UButton
                color="neutral"
                :variant="menu.active ? 'soft' : 'ghost'"
                size="sm"
                trailing-icon="i-hugeicons-arrow-down-01"
                :label="menu.label"
                :disabled="menu.items.length === 0"
                :data-testid="`review-menu-${menu.id}`"
              />
              <template #item-leading="{ item }">
                <span
                  v-if="menu.id === 'label'"
                  class="size-3 shrink-0 rounded-full"
                  :style="{ backgroundColor: labelStyle(String(item.color)).backgroundColor }"
                  aria-hidden="true"
                />
              </template>
            </UDropdownMenu>
            <UDropdownMenu :items="sortItems" :content="{ align: 'end', side: 'bottom' }">
              <UButton
                color="neutral"
                variant="ghost"
                size="sm"
                icon="i-hugeicons-sorting-05"
                trailing-icon="i-hugeicons-arrow-down-01"
                :label="sortLabel"
                data-testid="review-menu-sort"
              />
            </UDropdownMenu>
          </div>
        </div>

        <div v-if="loading" class="text-muted m-auto flex items-center gap-2 p-6 text-sm">
          <UIcon name="i-hugeicons-loading-03" class="size-4 animate-spin" />
          Loading changes…
        </div>
        <div v-else-if="error" class="p-3">
          <UAlert color="error" variant="subtle" icon="i-hugeicons-alert-circle" :description="error" role="alert" />
        </div>
        <UEmpty
          v-else-if="rows.length === 0 && searching"
          class="m-auto p-6"
          variant="naked"
          icon="i-hugeicons-search-remove"
          title="No changes match your search."
          data-testid="review-empty-search"
        >
          <template #actions>
            <UButton
              color="neutral"
              variant="outline"
              size="sm"
              label="Clear filters"
              data-testid="review-clear-filters"
              @click="setQ(`is:${query.state}`)"
            />
          </template>
        </UEmpty>
        <UEmpty
          v-else-if="rows.length === 0"
          class="m-auto p-6"
          variant="naked"
          icon="i-hugeicons-task-done-01"
          :title="query.state === 'closed' ? 'No closed changes.' : 'No changes waiting for review.'"
          data-testid="review-empty"
        />
        <ul v-else class="divide-default m-0 flex list-none flex-col divide-y overflow-y-auto p-0">
          <li
            v-for="item in rows"
            :key="item.number"
            class="hover:bg-elevated/50 flex cursor-pointer items-start gap-3 px-4 py-3"
            data-testid="review-item"
            @click="open(item.number)"
          >
            <UIcon
              :name="prIcon(item).name"
              :class="['mt-0.5 size-4 shrink-0', prIcon(item).class]"
              :aria-label="prIcon(item).label"
              :data-state="item.state ?? 'open'"
              data-testid="review-item-icon"
            />
            <div class="flex min-w-0 flex-1 flex-col gap-1">
              <div class="flex min-w-0 flex-wrap items-center gap-1.5">
                <RouterLink
                  :to="reviewPath(projectId, item.number)"
                  class="text-highlighted min-w-0 font-medium hover:underline"
                  data-testid="review-item-title"
                  @click.stop
                >{{ item.title }}</RouterLink>
                <span
                  v-for="label in item.labels ?? []"
                  :key="label.name"
                  class="rounded-full px-2 py-0.5 text-xs leading-4 font-medium"
                  :style="labelStyle(label.color)"
                  data-testid="review-item-label"
                >{{ label.name }}</span>
              </div>
              <p class="text-muted flex flex-wrap items-center gap-1 text-xs" data-testid="review-item-meta">
                <span>#{{ item.number }}</span>
                <span>·</span>
                <span>{{ item.author }} opened {{ relativeTime(item.createdAt ?? item.updatedAt) }}</span>
                <template v-if="item.checks !== 'none'">
                  <span>·</span>
                  <span
                    class="flex items-center gap-0.5"
                    :data-checks="item.checks"
                    data-testid="review-checks"
                  >
                    <UIcon
                      :name="CHECKS[item.checks].icon"
                      :class="['size-3.5', CHECKS[item.checks].class]"
                      :aria-label="CHECKS[item.checks].label"
                    />
                    <span v-if="item.checksTotal" class="tabular-nums">{{ item.checksPassed ?? 0 }}/{{ item.checksTotal }}</span>
                  </span>
                </template>
              </p>
            </div>
            <div class="flex shrink-0 items-center gap-3">
              <span
                v-if="item.comments"
                class="text-muted flex items-center gap-1 text-xs tabular-nums"
                :aria-label="`${item.comments} comments`"
                data-testid="review-comments"
              >
                <UIcon name="i-hugeicons-message-01" class="size-4" />
                {{ item.comments }}
              </span>
              <UBadge v-if="item.mine" variant="soft" color="neutral" label="Yours" data-testid="review-mine" />
              <UBadge
                v-else-if="item.state !== 'closed' && item.state !== 'merged'"
                variant="soft"
                :color="STATE[item.myState].color"
                :label="STATE[item.myState].label"
              />
            </div>
          </li>
        </ul>
      </div>
    </div>
  </div>
</template>
