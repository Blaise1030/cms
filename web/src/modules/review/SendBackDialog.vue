<script setup lang="ts">
import { computed, ref, watch } from "vue";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UFormField from "@nuxt/ui/components/FormField.vue";
import UModal from "@nuxt/ui/components/Modal.vue";
import UTextarea from "@nuxt/ui/components/Textarea.vue";

const open = defineModel<boolean>("open", { default: false });
const props = defineProps<{ busy: boolean; error?: string | null }>();
const emit = defineEmits<{ submit: [comment: string] }>();

const comment = ref("");
const valid = computed(() => comment.value.trim().length > 0);

watch(open, (isOpen) => {
  if (isOpen) comment.value = "";
});

function submit() {
  if (!valid.value || props.busy) return;
  emit("submit", comment.value.trim());
}
</script>

<template>
  <UModal v-model:open="open" title="Send back" description="Tell the author what to change. They see this note on their change.">
    <template #body>
      <form class="grid gap-4" @submit.prevent="submit">
        <UFormField label="What needs to change?" name="comment" required>
          <UTextarea
            v-model="comment"
            class="w-full"
            :rows="4"
            autoresize
            :maxrows="10"
            placeholder="What should be different before this goes live"
            aria-label="What needs to change"
            data-testid="send-back-comment"
            autofocus
            @keydown.meta.enter.prevent="submit"
            @keydown.ctrl.enter.prevent="submit"
          />
        </UFormField>
        <UAlert
          v-if="error"
          color="error"
          variant="subtle"
          icon="i-hugeicons-alert-circle"
          :description="error"
          role="alert"
          data-testid="send-back-error"
        />
        <div class="flex justify-end gap-2">
          <UButton type="button" color="neutral" variant="outline" label="Cancel" @click="open = false" />
          <UButton
            type="submit"
            :disabled="!valid || busy"
            :label="busy ? 'Sending back…' : 'Send back'"
            data-testid="send-back-submit"
          />
        </div>
      </form>
    </template>
  </UModal>
</template>
