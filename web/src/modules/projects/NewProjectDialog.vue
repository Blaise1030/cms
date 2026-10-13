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
import { errorMessage } from "@workbench/utils";
import type { Project } from "@/core/types";
import { useProjectsStore } from "./store";

const open = defineModel<boolean>("open", { default: false });
const emit = defineEmits<{ created: [project: Project] }>();
const store = useProjectsStore();

const name = ref("");
const error = ref<string | null>(null);

watch(open, (isOpen) => {
  if (isOpen) {
    name.value = "";
    error.value = null;
  }
});

async function submit() {
  if (!name.value.trim() || store.creating) return;
  error.value = null;
  try {
    const project = await store.create(name.value);
    emit("created", project);
    open.value = false;
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
          <DialogTitle>New project</DialogTitle>
          <DialogDescription>
            Scaffolds a marketing site from the template and starts its dev server.
          </DialogDescription>
        </DialogHeader>
        <Input v-model="name" placeholder="Project name" aria-label="Project name" autofocus />
        <p v-if="error" class="text-destructive text-sm" role="alert">{{ error }}</p>
        <DialogFooter>
          <Button type="button" variant="outline" @click="open = false">Cancel</Button>
          <Button type="submit" :disabled="!name.trim() || store.creating">
            {{ store.creating ? "Creating…" : "Create project" }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
