<script setup lang="ts">
// The "Set up your assistant" dialog. It is mounted once in App.vue and opened
// through `openAssistantSetup()`, so every entry point shares one dialog.
import UModal from "@nuxt/ui/components/Modal.vue";
import AssistantSetup from "./AssistantSetup.vue";
import { assistantDialogOpen, closeAssistantSetup } from "./dialog";

const emit = defineEmits<{
  /** The user picked a ready assistant; the caller opens a chat with it. */
  connect: [id: string];
}>();

/** Forwarded to the dialog panel, so the testid sits on the same element as before. */
const content = { "data-testid": "assistant-dialog" } as Record<string, string>;

function onOpenChange(open: boolean) {
  assistantDialogOpen.value = open;
}

function onContinue() {
  closeAssistantSetup();
}

function onConnect(id: string) {
  emit("connect", id);
  closeAssistantSetup();
}
</script>

<template>
  <UModal
    :open="assistantDialogOpen"
    title="Set up your assistant"
    description="Download one here, then start chatting."
    :content="content"
    :ui="{ content: 'sm:max-w-lg' }"
    @update:open="onOpenChange"
  >
    <template #body>
      <AssistantSetup show-continue @continue="onContinue" @connect="onConnect" />
    </template>
  </UModal>
</template>
