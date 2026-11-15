<script setup lang="ts">
import { toast } from "@/lib/toast";
import type { ModalProps } from "@nuxt/ui";
import UButton from "@nuxt/ui/components/Button.vue";
import UModal from "@nuxt/ui/components/Modal.vue";
import { errorMessage } from "@orso/utils";
import { useChangesStore } from "./store";

const store = useChangesStore();
/** The testid sits on the dialog panel itself, like before. */
const content = { "data-testid": "confirm-switch" } as ModalProps["content"];

async function confirm() {
  try {
    await store.confirmSwitch();
  } catch (e) {
    toast.error(errorMessage(e));
  }
}
</script>

<template>
  <UModal
    :open="store.pendingSwitch !== null"
    :close="false"
    :content="content"
    @update:open="(open) => !open && store.cancelSwitch()"
  >
    <template v-if="store.pendingSwitch" #title>
      You switched drafts — this chat was on {{ store.pendingSwitch.fromTitle }}. Continue on
      {{ store.pendingSwitch.toTitle }}?
    </template>
    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton
          color="neutral"
          variant="outline"
          label="Cancel"
          data-testid="confirm-switch-cancel"
          @click="store.cancelSwitch()"
        />
        <UButton
          label="Continue"
          data-testid="confirm-switch-continue"
          :disabled="store.switching"
          @click="confirm"
        />
      </div>
    </template>
  </UModal>
</template>
