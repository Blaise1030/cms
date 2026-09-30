<script setup lang="ts">
import ToolCard from "./ToolCard.vue";
import type { TurnView } from "./types";

defineProps<{ turn: TurnView }>();
</script>

<template>
  <div class="flex flex-col gap-2" data-testid="chat-turn">
    <div v-if="turn.userText" class="flex justify-end">
      <div
        class="bg-primary text-primary-foreground max-w-[85%] rounded-lg px-3 py-2 text-sm whitespace-pre-wrap"
        data-testid="user-text"
      >
        {{ turn.userText }}
      </div>
    </div>
    <div class="flex flex-col gap-2">
      <ToolCard v-for="tool in turn.tools" :key="tool.id" :tool="tool" />
      <div
        v-if="turn.assistantText"
        class="max-w-[95%] text-sm leading-relaxed whitespace-pre-wrap"
        data-testid="assistant-text"
      >
        {{ turn.assistantText }}
      </div>
      <p
        v-if="turn.status === 'streaming' && !turn.assistantText && turn.tools.length === 0"
        class="text-muted-foreground text-xs"
      >
        Thinking…
      </p>
      <p
        v-if="turn.status === 'done' && turn.stopReason === 'cancelled'"
        class="text-muted-foreground text-xs italic"
      >
        Stopped
      </p>
      <p v-if="turn.error" class="text-destructive text-sm" role="alert" data-testid="turn-error">
        {{ turn.error }}
      </p>
    </div>
  </div>
</template>
