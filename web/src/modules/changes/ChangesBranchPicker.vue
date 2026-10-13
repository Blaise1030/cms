<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Check, ChevronsUpDown, GitBranch, Plus } from "@lucide/vue";
import { toast } from "vue-sonner";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { errorMessage } from "@workbench/utils";
import NewChangeDialog from "./NewChangeDialog.vue";
import { PR_STATE_LABEL } from "./prStatus";
import { useChangesStore } from "./store";

const props = defineProps<{ projectId: string }>();
const store = useChangesStore();

const open = ref(false);
const showNewChange = ref(false);
const query = ref("");

watch(open, async (isOpen) => {
  if (!isOpen) {
    query.value = "";
    return;
  }
  try {
    await store.list(props.projectId);
  } catch (e) {
    toast.error(errorMessage(e));
  }
});

const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return store.items;
  return store.items.filter(
    (c) => c.title.toLowerCase().includes(q) || c.branch.toLowerCase().includes(q),
  );
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

function onNewChange() {
  open.value = false;
  showNewChange.value = true;
}
</script>

<template>
  <DropdownMenu v-model:open="open">
    <DropdownMenuTrigger as-child>
      <Button
        variant="secondary"
        size="sm"
        class="h-7 max-w-56 gap-1 px-2 text-xs"
        data-testid="changes-menu"
      >
        <GitBranch class="size-4 shrink-0 opacity-70" />
        <span class="truncate" data-testid="active-change">{{ triggerLabel }}</span>
        <ChevronsUpDown class="size-3.5 shrink-0 opacity-50" />
      </Button>
    </DropdownMenuTrigger>
    <DropdownMenuContent
      align="start"
      class="w-72 p-0"
      data-testid="changes-menu-content"
      @close-auto-focus.prevent
    >
      <div class="p-2">
        <Input
          v-model="query"
          placeholder="Search changes…"
          class="h-8"
          data-testid="changes-search"
          @keydown.stop
        />
      </div>
      <DropdownMenuSeparator class="my-0" />
      <div class="max-h-64 overflow-y-auto p-1">
        <p v-if="store.listing" class="text-muted-foreground px-2 py-1.5 text-xs">Loading…</p>
        <p
          v-else-if="filtered.length === 0"
          class="text-muted-foreground px-2 py-1.5 text-xs"
        >
          {{ query.trim() ? "No matching changes" : "No open changes" }}
        </p>
        <DropdownMenuItem
          v-for="c in filtered"
          :key="c.branch"
          class="flex items-start gap-2"
          data-testid="change-item"
          :data-branch="c.branch"
          :disabled="store.switching"
          @select.prevent="onSelect(c.branch)"
        >
          <Check
            class="mt-0.5 size-3.5 shrink-0"
            :class="c.branch === store.active?.branch ? 'opacity-100' : 'opacity-0'"
          />
          <span class="min-w-0 flex-1">
            <span class="flex items-center gap-1.5">
              <span class="truncate text-sm">{{ c.title }}</span>
              <span
                v-if="c.prState"
                class="text-muted-foreground ml-auto shrink-0 rounded border px-1 text-[10px] leading-4"
                :data-state="c.prState"
                data-testid="change-pr-state"
                >{{ PR_STATE_LABEL[c.prState] }}</span
              >
            </span>
            <span class="text-muted-foreground block truncate text-xs">
              {{ c.branch }}{{ c.lastEdited != null ? ` · ${formatEdited(c.lastEdited)}` : "" }}
            </span>
          </span>
        </DropdownMenuItem>
      </div>
      <DropdownMenuSeparator class="my-0" />
      <div class="p-1">
        <DropdownMenuItem data-testid="new-change" @select="onNewChange">
          <Plus class="size-3.5" />
          New change
        </DropdownMenuItem>
      </div>
    </DropdownMenuContent>
  </DropdownMenu>

  <NewChangeDialog
    v-model:open="showNewChange"
    :project-id="projectId"
    via-agent
    description="The agent starts it from the latest version of the site."
  />
</template>
