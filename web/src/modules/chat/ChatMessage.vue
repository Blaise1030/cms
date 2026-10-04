<script setup lang="ts">
import { computed } from "vue";
import { Message, MessageContent, MessageResponse } from "@/components/ai-elements/message";
import { parseElementRefs } from "@/modules/preview/elementPicker";
import { ELEMENT_CHIP_CLASS } from "./elementRef";
import ToolCard from "./ToolCard.vue";
import type { TurnView } from "./types";

const props = defineProps<{ turn: TurnView }>();

// Picked preview elements travel as text references; show them as the chips they were typed as.
const userParts = computed(() => parseElementRefs(props.turn.userText ?? ""));
</script>

<template>
  <div class="flex flex-col gap-3" data-testid="chat-turn">
    <Message v-if="turn.userText" from="user">
      <MessageContent>
        <p class="whitespace-pre-wrap [overflow-wrap:anywhere]" data-testid="user-text">
          <template v-for="(part, i) in userParts" :key="i">
            <span
              v-if="'element' in part"
              :class="ELEMENT_CHIP_CLASS"
              :title="part.element.prompt"
              data-testid="user-element-ref"
            >&lt;{{ part.element.label }}&gt;</span>
            <template v-else>{{ part.text }}</template>
          </template>
        </p>
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
          <MessageResponse
            v-if="turn.assistantText"
            :content="turn.assistantText"
            class="leading-relaxed"
            data-testid="assistant-text"
          />
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
