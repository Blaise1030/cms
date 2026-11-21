<script setup lang="ts">
import { computed } from "vue";
import { toast } from "@/lib/toast";
import type { DropdownMenuItem } from "@nuxt/ui";
import UButton from "@nuxt/ui/components/Button.vue";
import UDropdownMenu from "@nuxt/ui/components/DropdownMenu.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import type { ChecksState, PrState, PrStatus } from "@/core/types";
import { errorMessage } from "@orso/utils";
import { prStatusLabel } from "./prStatus";
import { useChangesStore } from "./store";

const props = defineProps<{ variant: "default" | "secondary" }>();

const store = useChangesStore();
const status = computed(() => store.status);
/** The deployed preview, only if it is a plain web URL (never `javascript:` and the like). */
const previewHref = computed(() => {
  const url = status.value?.previewUrl;
  return url && /^https?:\/\/./i.test(url) ? url : null;
});

type StatusIcon = { name: string; class: string };

const CHECK_ICON: Record<Exclude<ChecksState, "none">, StatusIcon> = {
  pending: { name: "i-hugeicons-clock-01", class: "text-warning animate-pulse" },
  passing: { name: "i-hugeicons-checkmark-circle-02", class: "text-success" },
  failing: { name: "i-hugeicons-cancel-circle", class: "text-error" },
};

const STATE_ICON: Record<PrState, StatusIcon> = {
  draft: { name: "i-hugeicons-dashed-line-circle", class: "text-muted" },
  open: { name: "i-hugeicons-record", class: "text-info" },
  approved: { name: "i-hugeicons-checkmark-badge-01", class: "text-success" },
  changes_requested: { name: "i-hugeicons-arrow-turn-backward", class: "text-warning" },
  merged: { name: "i-hugeicons-checkmark-circle-02", class: "text-success" },
  closed: { name: "i-hugeicons-cancel-circle", class: "text-muted" },
};

/** A terminal PR shows its outcome; an active one shows its checks when it has any. */
function statusIcon(s: Pick<PrStatus, "state" | "checks">) {
  if (s.state === "merged" || s.state === "closed") return STATE_ICON[s.state];
  return s.checks === "none" ? STATE_ICON[s.state] : CHECK_ICON[s.checks];
}
const icon = computed(() => (status.value ? statusIcon(status.value) : null));

/**
 * Route a menu link out to the browser ourselves. Selecting an item closes the menu, which
 * unmounts the anchor before WebKit runs its `target="_blank"` navigation and drops the click
 * (the desktop shell's new-window handler opens whatever `window.open` asks for).
 */
function openExternal(e: MouseEvent) {
  const href = (e.currentTarget as HTMLAnchorElement | null)?.href ?? "";
  if (!/^https?:\/\//i.test(href)) return;
  e.preventDefault();
  window.open(href, "_blank", "noopener,noreferrer");
}

async function onReady() {
  const id = store.projectId;
  const branch = store.active?.branch;
  if (!id || !branch) return;
  try {
    await store.markReady(id, branch);
  } catch (e) {
    // An expired session shows the sign-in prompt instead.
    if (!store.needsSignIn) toast.error(errorMessage(e));
  }
}

const items = computed<DropdownMenuItem[][]>(() => {
  const groups: DropdownMenuItem[][] = [];
  if (status.value) groups.push([{ type: "label", slot: "status" as const }]);
  const links: DropdownMenuItem[] = [
    {
      label: "Open on GitHub",
      icon: "i-hugeicons-git-pull-request",
      href: status.value?.url ?? store.prUrl ?? undefined,
      target: "_blank",
      "data-testid": "pr-link",
      onClick: openExternal,
    },
  ];
  if (previewHref.value) {
    links.push({
      label: "Open deployed preview",
      icon: "i-hugeicons-globe-02",
      href: previewHref.value,
      target: "_blank",
      "data-testid": "preview-deploy-link",
      onClick: openExternal,
    });
  }
  groups.push(links);
  if (status.value?.state === "draft") {
    groups.push([
      {
        label: store.markingReady ? "Asking…" : "Ask for a greenlight",
        icon: "i-hugeicons-checkmark-circle-02",
        disabled: store.markingReady,
        "data-testid": "ready-for-review",
        onSelect: () => void onReady(),
      },
    ]);
  }
  return groups;
});
</script>

<template>
  <!-- Links out show their arrow at the item's end, not straight after the label. -->
  <UDropdownMenu :items="items" :content="{ align: 'end' }" :ui="{ content: 'min-w-56' }" :external-icon="false">
    <UButton
      :color="props.variant === 'default' ? 'primary' : 'neutral'"
      :variant="props.variant === 'default' ? 'solid' : 'outline'"
      size="sm"
      icon="i-hugeicons-task-done-01"
      trailing-icon="i-hugeicons-arrow-down-01"
      :ui="{ trailingIcon: 'size-3.5 opacity-60' }"
      data-testid="pr-menu"
    >
      Review
      <UIcon
        v-if="icon"
        :name="icon.name"
        class="size-3.5 shrink-0"
        :class="icon.class"
        data-testid="pr-trigger-icon"
        aria-hidden="true"
      />
    </UButton>
    <template #item-trailing="{ item }">
      <UIcon
        v-if="item.target === '_blank'"
        name="i-hugeicons-link-square-02"
        class="text-dimmed size-3.5 shrink-0"
        data-testid="pr-menu-external"
        aria-hidden="true"
      />
    </template>
    <template #status>
      <span
        v-if="status"
        class="text-muted flex items-center gap-2 text-xs font-normal"
        data-testid="pr-status"
        :data-state="status.state"
        :data-checks="status.checks"
      >
        <UIcon
          v-if="icon"
          :name="icon.name"
          class="size-3.5 shrink-0"
          :class="icon.class"
          data-testid="pr-status-icon"
          aria-hidden="true"
        />
        {{ prStatusLabel(status) }}
      </span>
      <div
        v-if="status?.state === 'changes_requested' && status.sentBack"
        class="text-default mt-1.5 max-w-64 space-y-0.5 text-xs font-normal"
      >
        <p class="text-muted" data-testid="pr-sent-back-reviewer">Sent back by {{ status.sentBack.reviewer }}</p>
        <p class="whitespace-pre-wrap" data-testid="pr-sent-back-comment">{{ status.sentBack.comment }}</p>
      </div>
    </template>
  </UDropdownMenu>
</template>
