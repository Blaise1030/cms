<script setup lang="ts">
import { ref, watch } from "vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UInput from "@nuxt/ui/components/Input.vue";
import UModal from "@nuxt/ui/components/Modal.vue";
import { errorMessage } from "@orso/utils";
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
  <UModal
    v-model:open="open"
    title="New project"
    description="Scaffolds a marketing site from the template and starts its dev server."
  >
    <template #body>
      <form id="new-project-form" class="grid gap-2" @submit.prevent="submit">
        <UInput
          v-model="name"
          class="w-full"
          placeholder="Project name"
          aria-label="Project name"
          autofocus
        />
        <p v-if="error" class="text-error text-sm" role="alert">{{ error }}</p>
      </form>
    </template>
    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton type="button" color="neutral" variant="outline" label="Cancel" @click="open = false" />
        <UButton
          type="submit"
          form="new-project-form"
          :loading="store.creating"
          :disabled="!name.trim() || store.creating"
          :label="store.creating ? 'Creating…' : 'Create project'"
        />
      </div>
    </template>
  </UModal>
</template>
