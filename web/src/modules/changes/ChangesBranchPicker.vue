<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { toast } from "@/lib/toast";
import type { CommandPaletteGroup, CommandPaletteItem, InputProps, PopoverProps } from "@nuxt/ui";
import UBadge from "@nuxt/ui/components/Badge.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UCommandPalette from "@nuxt/ui/components/CommandPalette.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import UPopover from "@nuxt/ui/components/Popover.vue";
import { errorMessage } from "@orso/utils";
import NewChangeDialog from "./NewChangeDialog.vue";
import ConfirmSwitchDialog from "./ConfirmSwitchDialog.vue";
import { PR_STATE_LABEL } from "./prStatus";
import { useChangesStore } from "./store";

const props = defineProps<{ projectId: string }>();
const store = useChangesStore();

const open = ref(false);
const showNewChange = ref(false);

watch(open, async (isOpen) => {
  if (!isOpen) return;
  try {
    await store.list(props.projectId);
  } catch (e) {
    toast.error(errorMessage(e));
  }
});

const triggerLabel = computed(() => store.active?.title ?? "main");

function formatEdited(ts?: number): string {
  if (ts == null) return "";
  const diffSec = Math.max(0, Math.floor(Date.now() / 1000 - ts));
  if (diffSec < 60) return "just now";
  if (diffSec < 3600) return `${Math.floor(diffSec / 60)}m ago`;
  if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ago`;
  return `${Math.floor(diffSec / 86400)}d ago`;
}

async function onSelect(branch: string) {
  if (store.active?.branch === branch) {
    open.value = false;
    return;
  }
  try {
    await store.switchTo(props.projectId, branch);
    open.value = false;
  } catch (e) {
    toast.error(errorMessage(e));
  }
}

async function onLiveSite() {
  if (!store.active) {
    open.value = false;
    return;
  }
  try {
    await store.goLive(props.projectId);
    open.value = false;
  } catch (e) {
    toast.error(errorMessage(e));
  }
}

function onNewChange() {
  open.value = false;
  showNewChange.value = true;
}

/** Popover panel and search field keep the testids the e2e specs drive. */
const popoverContent = { align: "start", "data-testid": "changes-menu-content" } as PopoverProps["content"];
const searchInput = { "data-testid": "changes-search" } as Omit<InputProps, "modelValue" | "defaultValue">;

function prStateLabel(state: keyof typeof PR_STATE_LABEL): string {
  return PR_STATE_LABEL[state];
}

/** A row in the menu: Live site, an open change, or the list's loading/empty note. */
interface ChangeRow extends CommandPaletteItem {
  checked?: boolean;
  prState?: keyof typeof PR_STATE_LABEL;
}

const groups = computed<CommandPaletteGroup<ChangeRow>[]>(() => {
  const busy = store.switching || store.cleaningUp;
  const rows: ChangeRow[] = [
    {
      label: "Live site",
      description: "main",
      slot: "change",
      checked: !store.active,
      disabled: busy,
      "data-testid": "live-site",
      onSelect: () => void onLiveSite(),
    },
  ];
  if (store.listing || store.items.length === 0) {
    rows.push({ label: store.listing ? "Loading…" : "No open changes", slot: "note", disabled: true });
  }
  for (const c of store.items) {
    rows.push({
      label: c.title,
      // The branch is searchable too, as before.
      description: `${c.branch}${c.lastEdited != null ? ` · ${formatEdited(c.lastEdited)}` : ""}`,
      slot: "change",
      checked: c.branch === store.active?.branch,
      prState: c.prState ?? undefined,
      disabled: busy,
      "data-testid": "change-item",
      "data-branch": c.branch,
      onSelect: () => void onSelect(c.branch),
    });
  }
  return [
    { id: "changes", items: rows },
    {
      id: "actions",
      items: [{ label: "New change", icon: "i-hugeicons-plus-sign", "data-testid": "new-change", onSelect: onNewChange }],
    },
  ];
});
</script>

<template>
  <UPopover v-model:open="open" :content="popoverContent" :ui="{ content: 'w-72' }">
    <UButton
      color="neutral"
      variant="outline"
      size="sm"
      icon="i-hugeicons-git-branch"
      trailing-icon="i-hugeicons-unfold-more"
      class="max-w-56"
      :ui="{ leadingIcon: 'text-muted', trailingIcon: 'size-3.5 text-dimmed' }"
      data-testid="changes-menu"
    >
      <span class="truncate" data-testid="active-change">{{ triggerLabel }}</span>
    </UButton>

    <template #content>
      <UCommandPalette
        :groups="groups"
        placeholder="Search changes…"
        :input="searchInput"
        size="sm"
        :ui="{ viewport: 'max-h-64 divide-y divide-default' }"
      >
        <!-- Label and description come from the palette's own item layout; only the check and the
             PR badge are custom. -->
        <template #change-leading="{ item }">
          <UIcon
            name="i-hugeicons-checkmark-circle-02"
            class="text-primary mt-0.5 size-4 shrink-0"
            :class="item.checked ? 'opacity-100' : 'opacity-0'"
          />
        </template>
        <template #change-trailing="{ item }">
          <UBadge
            v-if="item.prState"
            color="neutral"
            variant="outline"
            size="sm"
            class="shrink-0"
            :data-state="item.prState"
            data-testid="change-pr-state"
            :label="prStateLabel(item.prState)"
          />
        </template>
        <template #note="{ item }">
          <span class="text-muted text-xs">{{ item.label }}</span>
        </template>
        <template #empty>
          <span class="text-sm">No matching changes</span>
        </template>
      </UCommandPalette>
    </template>
  </UPopover>

  <NewChangeDialog v-model:open="showNewChange" :project-id="projectId" />

  <ConfirmSwitchDialog />
</template>
