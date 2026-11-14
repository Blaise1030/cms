<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ChevronsUpDown, CircleCheck, GitBranch, Plus } from "@lucide/vue";
import { toast } from "vue-sonner";
import { Button } from "@/components/ui/button";
import { Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList, CommandSeparator } from "@/components/ui/command";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
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
</script>

<template>
  <Popover v-model:open="open">
    <PopoverTrigger as-child>
      <Button
        variant="outline"
        size="sm"
        class="bg-card h-7 max-w-56 gap-1 px-2 text-xs"
        data-testid="changes-menu"
      >
        <GitBranch class="size-4 shrink-0 opacity-70" />
        <span class="truncate" data-testid="active-change">{{ triggerLabel }}</span>
        <ChevronsUpDown class="size-3.5 shrink-0 opacity-50" />
      </Button>
    </PopoverTrigger>
    <PopoverContent align="start" class="w-72 p-0" data-testid="changes-menu-content">
      <Command>
        <CommandInput placeholder="Search changes…" data-testid="changes-search" />
        <CommandList class="max-h-64">
          <CommandEmpty>No matching changes</CommandEmpty>
          <CommandGroup>
            <CommandItem
              value="live-site main"
              data-testid="live-site"
              :disabled="store.switching || store.cleaningUp"
              class="items-start"
              @select="onLiveSite"
            >
              <CircleCheck
                class="mt-0.5 size-3.5 shrink-0 fill-foreground text-background"
                :class="store.active ? 'opacity-0' : 'opacity-100'"
              />
              <span class="min-w-0 flex-1">
                <span class="truncate text-sm">Live site</span>
                <span class="text-muted-foreground block truncate text-xs">main</span>
              </span>
            </CommandItem>
            <p v-if="store.listing" class="text-muted-foreground px-2 py-1.5 text-xs">Loading…</p>
            <p
              v-else-if="store.items.length === 0"
              class="text-muted-foreground px-2 py-1.5 text-xs"
            >
              No open changes
            </p>
            <CommandItem
              v-for="c in store.items"
              :key="c.branch"
              :value="`${c.title} ${c.branch}`"
              data-testid="change-item"
              :data-branch="c.branch"
              :disabled="store.switching || store.cleaningUp"
              class="items-start"
              @select="onSelect(c.branch)"
            >
              <CircleCheck
                class="mt-0.5 size-3.5 shrink-0 fill-foreground text-background"
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
            </CommandItem>
          </CommandGroup>
          <CommandSeparator />
          <CommandGroup>
            <CommandItem value="new change" data-testid="new-change" @select="onNewChange">
              <Plus class="size-3.5" />
              New change
            </CommandItem>
          </CommandGroup>
        </CommandList>
      </Command>
    </PopoverContent>
  </Popover>

  <NewChangeDialog v-model:open="showNewChange" :project-id="projectId" />

  <ConfirmSwitchDialog />
</template>
