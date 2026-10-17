<script setup lang="ts">
import { defineAsyncComponent, ref, watch } from "vue";
import { agentPrompt, isRequest, useAgentOutboxStore, type AgentMessage } from "./outbox";
import type ChatPanelComponent from "./ChatPanel.vue";

/** Its AI SDK, markdown and highlighter would push the app shell past the PWA precache limit. */
const ChatPanel = defineAsyncComponent(() => import("./ChatPanel.vue"));

const props = defineProps<{ projectId: string }>();

const outbox = useAgentOutboxStore();
/** Null until the lazy panel has loaded; the outbox waits for it. */
const chat = ref<InstanceType<typeof ChatPanelComponent> | null>(null);

/** Picks land in the input and requests are sent one per turn; requests wait while the chat is busy. */
function deliverToChat() {
  const panel = chat.value;
  if (!panel || outbox.messages.length === 0) return;
  const held: AgentMessage[] = [];
  for (const message of outbox.drain()) {
    if (message.kind === "element") panel.insertReference(agentPrompt(message));
    else if (!isRequest(message)) panel.insert(agentPrompt(message));
    else if (panel.busy) held.push(message);
    else void panel.send(agentPrompt(message));
  }
  outbox.messages.push(...held);
}

watch([() => outbox.messages.length, chat, () => chat.value?.busy], deliverToChat, {
  flush: "post",
});
</script>

<template>
  <div class="bg-background h-full min-h-0" data-testid="agent-panel">
    <ChatPanel ref="chat" :project-id="props.projectId" :active="true" />
  </div>
</template>
