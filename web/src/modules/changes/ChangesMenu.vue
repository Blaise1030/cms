<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { errorMessage } from "@/lib/errors";
import { useChangesStore } from "./store";

const props = defineProps<{ projectId: string }>();
const store = useChangesStore();
const open = ref(false);
const menuError = ref<string | null>(null);

watch(open, async (isOpen) => {
  if (!isOpen) return;
  menuError.value = null;
  try {
    await store.list(props.projectId);
  } catch (e) {
    menuError.value = errorMessage(e);
  }
});

const empty = computed(() => !store.listing && store.items.length === 0 && !menuError.value);

function formatEdited(ts?: number): string {
  if (ts == null) return "";
  const diffSec = Math.max(0, Math.floor(Date.now() / 1000 - ts));
  if (diffSec < 60) return "just now";
  if (diffSec < 3600) {
    const m = Math.floor(diffSec / 60);
    return `${m}m ago`;
  }
  if (diffSec < 86400) {
    const h = Math.floor(diffSec / 3600);
    return `${h}h ago`;
  }
  const d = Math.floor(diffSec / 86400);
  return `${d}d ago`;
}

async function onSelect(branch: string) {
  menuError.value = null;
  try {
    await store.switchTo(props.projectId, branch);
    open.value = false;
  } catch (e) {
    menuError.value = errorMessage(e);
  }
}
</script>

<template>
  <DropdownMenu v-model:open="open">
    <DropdownMenuTrigger as-child>
      <Button variant="ghost" size="sm" class="h-7 px-2 text-xs" data-testid="changes-menu">
        Changes
      </Button>
    </DropdownMenuTrigger>
    <DropdownMenuContent align="start" class="w-64" data-testid="changes-menu-content">
      <DropdownMenuLabel>Open changes</DropdownMenuLabel>
      <DropdownMenuSeparator />
      <p v-if="store.listing" class="text-muted-foreground px-2 py-1.5 text-xs">Loading…</p>
      <p v-else-if="menuError" class="text-destructive px-2 py-1.5 text-xs" role="alert">
        {{ menuError }}
      </p>
      <p v-else-if="empty" class="text-muted-foreground px-2 py-1.5 text-xs">No open changes</p>
      <DropdownMenuItem
        v-for="c in store.items"
        :key="c.branch"
        class="flex flex-col items-start gap-0.5"
        data-testid="change-item"
        :data-branch="c.branch"
        :disabled="store.switching"
        @select.prevent="onSelect(c.branch)"
      >
        <span class="text-sm">{{ c.title }}</span>
        <span class="text-muted-foreground text-xs">{{ formatEdited(c.lastEdited) }}</span>
      </DropdownMenuItem>
    </DropdownMenuContent>
  </DropdownMenu>
</template>
