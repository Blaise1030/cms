<script setup lang="ts">
import { ref, watch } from "vue";
import { toast } from "@/lib/toast";
import UButton from "@nuxt/ui/components/Button.vue";
import UFormField from "@nuxt/ui/components/FormField.vue";
import UInput from "@nuxt/ui/components/Input.vue";
import UModal from "@nuxt/ui/components/Modal.vue";
import UTextarea from "@nuxt/ui/components/Textarea.vue";
import { errorMessage } from "@orso/utils";
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
    description: "Starts from the latest version of the site. Your assistant's saves go on this change.",
  },
);
const emit = defineEmits<{ created: [change: Change] }>();
const store = useChangesStore();

const title = ref("");
const details = ref("");

watch(open, (isOpen) => {
  if (isOpen) {
    title.value = "";
    details.value = "";
  }
});

async function submit() {
  if (!title.value.trim() || store.creating) return;
  try {
    const change = await store.create(props.projectId, title.value, details.value);
    // Emit before closing so parents can clear pending work without treating this as cancel.
    emit("created", change);
    open.value = false;
  } catch (e) {
    toast.error(errorMessage(e));
  }
}
</script>

<template>
  <UModal v-model:open="open" :title="heading" :description="description">
    <template #body>
      <!-- The form holds the actions too, so Enter in the field and the submit button both create. -->
      <form class="grid gap-4" @submit.prevent="submit">
        <UFormField label="Title" name="title" required>
          <UInput
            v-model="title"
            class="w-full"
            placeholder="New post: Launch week"
            aria-label="Change title"
            data-testid="new-change-title"
            autofocus
          />
        </UFormField>
        <UFormField
          label="Description"
          name="description"
          hint="Optional"
          help="Whoever greenlights this change sees it."
        >
          <UTextarea
            v-model="details"
            class="w-full"
            :rows="3"
            autoresize
            :maxrows="8"
            placeholder="What's changing and why"
            aria-label="Change description"
            data-testid="new-change-description"
            @keydown.meta.enter.prevent="submit"
            @keydown.ctrl.enter.prevent="submit"
          />
        </UFormField>
        <div class="flex justify-end gap-2">
          <UButton type="button" color="neutral" variant="outline" label="Cancel" @click="open = false" />
          <UButton
            type="submit"
            :disabled="!title.trim() || store.creating"
            :label="store.creating ? 'Creating…' : 'Create change'"
          />
        </div>
      </form>
    </template>
  </UModal>
</template>
