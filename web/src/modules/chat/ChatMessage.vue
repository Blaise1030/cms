<script setup lang="ts">
import { Message, MessageContent } from "@/components/ai-elements/message";
import ToolCard from "./ToolCard.vue";
import type { TurnView } from "./types";

defineProps<{ turn: TurnView }>();
</script>

<template>
  <div class="flex flex-col gap-3" data-testid="chat-turn">
    <Message v-if="turn.userText" from="user">
      <MessageContent>
        <p class="whitespace-pre-wrap" data-testid="user-text">{{ turn.userText }}</p>
      </MessageContent>
    </Message>

    <div class="flex max-w-[95%] flex-col gap-2">
      <ToolCard v-for="tool in turn.tools" :key="tool.id" :tool="tool" />

      <Message
        v-if="turn.assistantText || turn.status === 'streaming' || turn.error"
        from="assistant"
        class="max-w-full"
      >
        <MessageContent class="w-full">
          <p
            v-if="turn.assistantText"
            class="leading-relaxed whitespace-pre-wrap"
            data-testid="assistant-text"
          >
            {{ turn.assistantText }}
          </p>
          <p
            v-else-if="turn.status === 'streaming' && turn.tools.length === 0"
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
          <p
            v-if="turn.error"
            class="text-destructive text-sm"
            role="alert"
            data-testid="turn-error"
          >
            {{ turn.error }}
          </p>
        </MessageContent>
      </Message>
    </div>
  </div>
</template>
