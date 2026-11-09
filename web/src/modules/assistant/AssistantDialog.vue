<script setup lang="ts">
// The "Set up your assistant" dialog. It is mounted once in App.vue and opened
// through `openAssistantSetup()`, so every entry point shares one dialog.
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import AssistantSetup from "./AssistantSetup.vue";
import { assistantDialogOpen, closeAssistantSetup } from "./dialog";

const emit = defineEmits<{
  /** The user picked a ready assistant; the caller opens a chat with it. */
  connect: [id: string];
}>();

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
  <Dialog :open="assistantDialogOpen" @update:open="onOpenChange">
    <DialogContent class="sm:max-w-lg" data-testid="assistant-dialog">
      <DialogHeader>
        <DialogTitle>Set up your assistant</DialogTitle>
        <DialogDescription>Download one here, then start chatting.</DialogDescription>
      </DialogHeader>
      <AssistantSetup show-continue @continue="onContinue" @connect="onConnect" />
    </DialogContent>
  </Dialog>
</template>
