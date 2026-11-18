<script setup lang="ts">
import { useRouter } from "vue-router";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UBadge from "@nuxt/ui/components/Badge.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UEmpty from "@nuxt/ui/components/Empty.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import type { ChecksState, MyReviewState } from "@/core/types";
import { relativeTime } from "@/lib/relativeTime";
import { reviewPath } from "@/router";
import { useReviewStore } from "./store";
import { useReviewRefresh } from "./useReviewRefresh";

const props = defineProps<{ projectId: string }>();
const store = useReviewStore();
const router = useRouter();

// The rail polls too; this refreshes on focus and while the inbox is on screen.
useReviewRefresh(() => void store.load(props.projectId), () => props.projectId);

const STATE: Record<MyReviewState, { label: string; color: "primary" | "warning" | "success" }> = {
  new: { label: "New", color: "primary" },
  sent_back: { label: "You sent back", color: "warning" },
  greenlit: { label: "You greenlit", color: "success" },
};

const CHECKS: Record<Exclude<ChecksState, "none">, { icon: string; class: string; label: string }> = {
  pending: { icon: "i-hugeicons-clock-01", class: "text-warning animate-pulse", label: "Checks running" },
  passing: { icon: "i-hugeicons-checkmark-circle-02", class: "text-success", label: "Checks passing" },
  failing: { icon: "i-hugeicons-cancel-circle", class: "text-error", label: "Checks failing" },
};

const open = (number: number) => router.push(reviewPath(props.projectId, number));
</script>

<template>
  <div class="min-h-0 flex-1 overflow-y-auto" data-testid="review-inbox">
    <div class="p-3">
      <p v-if="store.loading && store.items.length === 0" class="text-muted flex items-center gap-2 text-sm">
        <UIcon name="i-hugeicons-loading-03" class="size-4 animate-spin" />
        Loading changes…
      </p>
      <UAlert
        v-else-if="store.inboxError && store.items.length === 0"
        color="error"
        variant="subtle"
        icon="i-hugeicons-alert-circle"
        :description="store.inboxError"
        role="alert"
      />
      <UEmpty
        v-else-if="store.items.length === 0"
        variant="naked"
        icon="i-hugeicons-task-done-01"
        title="Nothing waiting for your greenlight."
      />
      <ul v-else class="m-0 flex w-full list-none flex-col gap-1 p-0">
        <li v-for="item in store.items" :key="item.number">
          <UButton
            color="neutral"
            variant="ghost"
            class="w-full min-w-0 gap-2 px-2.5 py-2.5 text-left"
            data-testid="review-item"
            @click="open(item.number)"
          >
            <span class="flex min-w-0 flex-1 flex-col gap-0.5">
              <span class="text-highlighted truncate text-base font-medium">{{ item.title }}</span>
              <span class="text-muted text-xs font-normal">
                {{ item.author }} · {{ relativeTime(item.updatedAt) }}
              </span>
            </span>
            <span class="flex shrink-0 items-center gap-2">
              <UIcon
                v-if="item.checks !== 'none'"
                :name="CHECKS[item.checks].icon"
                :class="['size-4', CHECKS[item.checks].class]"
                :aria-label="CHECKS[item.checks].label"
                :data-checks="item.checks"
                data-testid="review-checks"
              />
              <UBadge variant="soft" :color="STATE[item.myState].color" :label="STATE[item.myState].label" />
              <UIcon name="i-hugeicons-arrow-right-01" class="text-muted size-4" />
            </span>
          </UButton>
        </li>
      </ul>
    </div>
  </div>
</template>
