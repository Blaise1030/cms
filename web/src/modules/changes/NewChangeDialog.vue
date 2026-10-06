<script setup lang="ts">
import { ref, watch } from "vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { errorMessage } from "@/lib/errors";
import type { Change } from "@/core/types";
import { useChangesStore } from "./store";

const open = defineModel<boolean>("open", { default: false });
const props = withDefaults(
  defineProps<{
    projectId: string;
    heading?: string;
    description?: string;
  }>(),
  {
    heading: "New change",
    description: "Creates a branch from the latest main. Agent checkpoints will commit to it.",
  },
);
const emit = defineEmits<{ created: [change: Change] }>();
const store = useChangesStore();

const title = ref("");
const error = ref<string | null>(null);

watch(open, (isOpen) => {
  if (isOpen) {
    title.value = "";
    error.value = null;
  }
});

async function submit() {
  if (!title.value.trim() || store.creating) return;
  error.value = null;
  try {
    const change = await store.create(props.projectId, title.value);
    open.value = false;
    emit("created", change);
  } catch (e) {
    error.value = errorMessage(e);
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent>
      <form class="grid gap-4" @submit.prevent="submit">
        <DialogHeader>
          <DialogTitle>{{ heading }}</DialogTitle>
          <DialogDescription>{{ description }}</DialogDescription>
        </DialogHeader>
        <Input
          v-model="title"
          placeholder="New post: Launch week"
          aria-label="Change title"
          autofocus
        />
        <p v-if="error" class="text-destructive text-sm" role="alert">{{ error }}</p>
        <DialogFooter>
          <Button type="button" variant="outline" @click="open = false">Cancel</Button>
          <Button type="submit" :disabled="!title.trim() || store.creating">
            {{ store.creating ? "Creating…" : "Create change" }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
