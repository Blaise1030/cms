// One shared open flag for the "Set up your assistant" dialog, mounted once in
// the app shell. Any surface (home header, command palette, chat) can open it
// without owning a copy of the dialog.
import { ref } from "vue";

export const assistantDialogOpen = ref(false);

export function openAssistantSetup() {
  assistantDialogOpen.value = true;
}

export function closeAssistantSetup() {
  assistantDialogOpen.value = false;
}
