<script setup lang="ts">
import { defineAsyncComponent, ref, watch } from "vue";
import { agentPrompt, isRequest, useAgentOutboxStore } from "./outbox";
import type ChatPanelComponent from "./ChatPanel.vue";

/** Its AI SDK, markdown and highlighter would push the app shell past the PWA precache limit. */
const ChatPanel = defineAsyncComponent(() => import("./ChatPanel.vue"));

const props = defineProps<{ projectId: string }>();

const outbox = useAgentOutboxStore();
/** Null until the lazy panel has loaded; the outbox waits for it. */
const chat = ref<InstanceType<typeof ChatPanelComponent> | null>(null);

/** Picks land in the input; requests go through the chat's visible per-chat queue. */
function deliverToChat() {
  const panel = chat.value;
  if (!panel || outbox.messages.length === 0) return;
  for (const message of outbox.drain()) {
    if (message.kind === "element") panel.insertReference(agentPrompt(message));
    else if (!isRequest(message)) panel.insert(agentPrompt(message));
    else void panel.send(agentPrompt(message));
  }
}

watch([() => outbox.messages.length, chat], deliverToChat, {
  flush: "post",
});
</script>

<template>
  <div class="bg-background h-full min-h-0" data-testid="agent-panel">
    <ChatPanel ref="chat" :project-id="props.projectId" :active="true" />
  </div>
</template>
