<script setup lang="ts">
import { ref } from "vue";
import { History, Trash2 } from "@lucide/vue";
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
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";

defineProps<{ chats: ChatSummary[]; activeId: string | null; runningIds: string[] }>();
const emit = defineEmits<{ pick: [id: string]; remove: [id: string] }>();
const open = defineModel<boolean>("open", { default: false });
const pendingDelete = ref<ChatSummary | null>(null);

const when = (ms: number) => new Date(ms).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
const titleOf = (c: ChatSummary) => c.title || "Untitled chat";

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
    <PopoverContent align="start" class="flex max-h-96 w-72 flex-col overflow-y-auto p-1.5">
      <nav aria-label="Chat history" data-testid="chat-history" class="flex flex-col gap-1">
        <p v-if="chats.length === 0" class="text-muted-foreground p-2 text-sm">No chats yet</p>
        <div
          v-for="c in chats"
          :key="c.id"
          class="group hover:bg-background hover:btn-raised-soft dark:hover:bg-accent/50 relative flex items-center rounded-md transition-all"
          :class="{ 'bg-primary/12 hover:bg-primary/16 btn-raised-soft dark:bg-primary/25 dark:hover:bg-primary/30': c.id === activeId }"
        >
          <button
            type="button"
            class="flex min-w-0 flex-1 flex-col items-start gap-0.5 px-2 py-1.5 text-left text-sm active:translate-y-px"
            :aria-current="c.id === activeId ? 'true' : undefined"
            data-testid="chat-history-item"
            @click="pick(c.id)"
          >
            <span class="w-full truncate">{{ titleOf(c) }}</span>
            <span class="text-muted-foreground text-xs">
              {{ when(c.updatedAt) }}<template v-if="runningIds.includes(c.id)"> · Running</template>
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
          “{{ pendingDelete && titleOf(pendingDelete) }}” and its messages will be removed. Changes the agent
          made to your site stay.
        </AlertDialogDescription>
      </AlertDialogHeader>
      <AlertDialogFooter>
        <AlertDialogCancel>Cancel</AlertDialogCancel>
        <Button variant="destructive" data-testid="chat-delete-confirm" @click="confirmDelete">Delete chat</Button>
      </AlertDialogFooter>
    </AlertDialogContent>
  </AlertDialog>
</template>
