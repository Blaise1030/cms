<script setup lang="ts">
import { History } from "@lucide/vue";
import type { ChatSummary } from "@/core/types";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";

defineProps<{ chats: ChatSummary[]; activeId: string | null; runningId: string | null }>();
const emit = defineEmits<{ pick: [id: string] }>();
const open = defineModel<boolean>("open", { default: false });

const when = (ms: number) => new Date(ms).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });

function pick(id: string) {
  open.value = false;
  emit("pick", id);
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
      <nav aria-label="Chat history" data-testid="chat-history" class="flex flex-col">
        <p v-if="chats.length === 0" class="text-muted-foreground p-2 text-sm">No chats yet</p>
        <button
          v-for="c in chats"
          :key="c.id"
          type="button"
          class="hover:bg-muted flex w-full flex-col items-start gap-0.5 rounded-md px-2 py-1.5 text-left text-sm"
          :class="{ 'bg-primary/10': c.id === activeId }"
          :aria-current="c.id === activeId ? 'true' : undefined"
          data-testid="chat-history-item"
          @click="pick(c.id)"
        >
          <span class="w-full truncate">{{ c.title || "Untitled chat" }}</span>
          <span class="text-muted-foreground text-xs">
            {{ when(c.updatedAt) }}<template v-if="runningId === c.id"> · Running</template>
          </span>
        </button>
      </nav>
    </PopoverContent>
  </Popover>
</template>
