<script setup lang="ts">
import { defineAsyncComponent, ref, watch } from "vue";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import TerminalPanel from "@/modules/terminal/TerminalPanel.vue";
import { agentPrompt, isRequest, useAgentOutboxStore, type AgentMessage } from "@/modules/chat/outbox";
import type ChatPanelComponent from "./ChatPanel.vue";

/** Its AI SDK, markdown and highlighter would push the app shell past the PWA precache limit. */
const ChatPanel = defineAsyncComponent(() => import("./ChatPanel.vue"));

const props = defineProps<{ projectId: string }>();

const outbox = useAgentOutboxStore();
const tab = ref<"chat" | "terminal">("chat");
/** Null until the lazy panel has loaded; the outbox waits for it. */
const chat = ref<InstanceType<typeof ChatPanelComponent> | null>(null);

/** In Chat, picks land in the input and requests are sent one per turn; requests wait while the chat is busy. */
function deliverToChat() {
  const panel = chat.value;
  if (tab.value !== "chat" || !panel || outbox.messages.length === 0) return;
  const held: AgentMessage[] = [];
  for (const message of outbox.drain()) {
    if (message.kind === "element") panel.insertReference(agentPrompt(message));
    else if (!isRequest(message)) panel.insert(agentPrompt(message));
    else if (panel.busy) held.push(message);
    else void panel.send(agentPrompt(message));
  }
  outbox.messages.push(...held);
}

watch([() => outbox.messages.length, tab, chat, () => chat.value?.busy], deliverToChat, {
  flush: "post",
});
</script>

<template>
  <Tabs v-model="tab" class="bg-background h-full min-h-0 gap-0" data-testid="agent-panel">
    <div class="flex shrink-0 p-1.5">
      <TabsList class="w-full">
        <TabsTrigger value="chat" data-testid="agent-tab-chat">Chat</TabsTrigger>
        <TabsTrigger value="terminal" data-testid="agent-tab-terminal">Terminal</TabsTrigger>
      </TabsList>
    </div>
    <TabsContent v-show="tab === 'chat'" value="chat" force-mount class="min-h-0">
      <ChatPanel ref="chat" :project-id="props.projectId" :active="tab === 'chat'" @open-terminal="tab = 'terminal'" />
    </TabsContent>
    <TabsContent v-show="tab === 'terminal'" value="terminal" force-mount class="min-h-0">
      <TerminalPanel :project-id="props.projectId" :active="tab === 'terminal'" />
    </TabsContent>
  </Tabs>
</template>
