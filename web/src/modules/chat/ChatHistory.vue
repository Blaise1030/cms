<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { History, MessageSquare, RefreshCw, Search, Trash2 } from "@lucide/vue";
import type { ChatSummary } from "@/core/types";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Spinner } from "@/components/ui/spinner";

const props = defineProps<{ chats: ChatSummary[]; activeId: string | null; runningIds: string[] }>();
const emit = defineEmits<{ pick: [id: string]; remove: [id: string] }>();
const open = defineModel<boolean>("open", { default: false });
const pendingDelete = ref<ChatSummary | null>(null);
const query = ref("");

watch(open, (isOpen) => {
  if (!isOpen) query.value = "";
});

const when = (ms: number) => new Date(ms).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
const titleOf = (c: ChatSummary) => c.title || "Untitled chat";
/** A sync chat still combining its draft with the latest site. */
const combining = (c: ChatSummary) => c.kind === "sync" && c.sync?.phase === "working";

type ChatStatus = "running" | "combining" | "idle";

function statusOf(c: ChatSummary): ChatStatus {
  if (props.runningIds.includes(c.id) || c.running) return "running";
  if (combining(c)) return "combining";
  return "idle";
}

const statusLabel: Record<ChatStatus, string> = {
  running: "Running",
  combining: "Combining",
  idle: "Idle",
};

const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return props.chats;
  return props.chats.filter((c) => titleOf(c).toLowerCase().includes(q));
});

function pick(id: string) {
  open.value = false;
  emit("pick", id);
}

function confirmDelete() {
  if (pendingDelete.value) emit("remove", pendingDelete.value.id);
  pendingDelete.value = null;
}
</script>

<template>
  <Popover v-model:open="open">
    <PopoverTrigger as-child>
      <Button
        :variant="open ? 'secondary' : 'ghost'"
        size="icon-sm"
        aria-label="Chat history"
        data-testid="chat-history-toggle"
      >
        <History />
      </Button>
    </PopoverTrigger>
    <PopoverContent align="start" class="flex w-72 flex-col p-0">
      <div class="border-b p-2">
        <InputGroup class="h-8">
          <InputGroupAddon align="inline-start">
            <Search class="size-4 opacity-50" aria-hidden="true" />
          </InputGroupAddon>
          <InputGroupInput
            v-model="query"
            placeholder="Search chats…"
            aria-label="Search chats"
            class="h-8"
            data-testid="chat-history-search"
            @keydown.stop
          />
        </InputGroup>
      </div>
      <nav
        aria-label="Chat history"
        data-testid="chat-history"
        class="flex max-h-72 flex-col gap-1 overflow-y-auto p-1.5"
      >
        <p v-if="chats.length === 0" class="text-muted-foreground p-2 text-sm">No chats yet</p>
        <p v-else-if="filtered.length === 0" class="text-muted-foreground p-2 text-sm">No matching chats</p>
        <div
          v-for="c in filtered"
          :key="c.id"
          class="group hover:bg-background hover:btn-raised-soft dark:hover:bg-accent/50 relative flex items-center rounded-md transition-all"
          :class="{ 'bg-primary/12 hover:bg-primary/16 btn-raised-soft dark:bg-primary/25 dark:hover:bg-primary/30': c.id === activeId }"
        >
          <button
            type="button"
            class="flex min-w-0 flex-1 items-start gap-2 px-2 py-1.5 text-left text-sm active:translate-y-px"
            :aria-current="c.id === activeId ? 'true' : undefined"
            :aria-label="`${titleOf(c)}, ${statusLabel[statusOf(c)]}`"
            data-testid="chat-history-item"
            :data-status="statusOf(c)"
            @click="pick(c.id)"
          >
            <span class="mt-0.5 shrink-0" :title="statusLabel[statusOf(c)]" data-testid="chat-history-status">
              <Spinner v-if="statusOf(c) === 'running'" class="size-3.5" />
              <RefreshCw
                v-else-if="statusOf(c) === 'combining'"
                class="text-muted-foreground size-3.5 animate-spin"
                aria-hidden="true"
              />
              <MessageSquare v-else class="text-muted-foreground size-3.5 opacity-70" aria-hidden="true" />
            </span>
            <span class="min-w-0 flex-1">
              <span class="block w-full truncate">{{ titleOf(c) }}</span>
              <span class="text-muted-foreground block truncate text-xs">{{ when(c.updatedAt) }}</span>
            </span>
          </button>
          <Button
            v-if="!runningIds.includes(c.id)"
            variant="ghost"
            size="icon-sm"
            class="text-muted-foreground hover:text-destructive mr-1 shrink-0 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
            :aria-label="`Delete ${titleOf(c)}`"
            data-testid="chat-history-delete"
            @click="pendingDelete = c"
          >
            <Trash2 />
          </Button>
        </div>
      </nav>
    </PopoverContent>
  </Popover>

  <AlertDialog :open="pendingDelete !== null" @update:open="(o) => !o && (pendingDelete = null)">
    <AlertDialogContent>
      <AlertDialogHeader>
        <AlertDialogTitle>Delete this chat?</AlertDialogTitle>
        <AlertDialogDescription>
          “{{ pendingDelete && titleOf(pendingDelete) }}” and its messages will be removed. Changes your
          assistant made to your site stay.
        </AlertDialogDescription>
      </AlertDialogHeader>
      <AlertDialogFooter>
        <AlertDialogCancel>Cancel</AlertDialogCancel>
        <Button variant="destructive" data-testid="chat-delete-confirm" @click="confirmDelete">Delete chat</Button>
      </AlertDialogFooter>
    </AlertDialogContent>
  </AlertDialog>
</template>
