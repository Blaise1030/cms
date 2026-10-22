<script setup lang="ts">
import { toast } from "vue-sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { errorMessage } from "@orso/utils";
import { useChangesStore } from "./store";

const store = useChangesStore();

async function confirm() {
  try {
    await store.confirmSwitch();
  } catch (e) {
    toast.error(errorMessage(e));
  }
}
</script>

<template>
  <Dialog
    :open="store.pendingSwitch !== null"
    @update:open="(open) => !open && store.cancelSwitch()"
  >
    <DialogContent v-if="store.pendingSwitch" :show-close-button="false" data-testid="confirm-switch">
      <DialogHeader>
        <DialogTitle>
          You switched drafts — this chat was on {{ store.pendingSwitch.fromTitle }}. Continue on
          {{ store.pendingSwitch.toTitle }}?
        </DialogTitle>
      </DialogHeader>
      <DialogFooter>
        <Button
          variant="outline"
          data-testid="confirm-switch-cancel"
          @click="store.cancelSwitch()"
        >
          Cancel
        </Button>
        <Button data-testid="confirm-switch-continue" :disabled="store.switching" @click="confirm">
          Continue
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
