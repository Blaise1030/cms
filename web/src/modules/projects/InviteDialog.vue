<script setup lang="ts">
import { computed } from "vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UModal from "@nuxt/ui/components/Modal.vue";
import { collaboratorsUrl } from "./invite";

const open = defineModel<boolean>("open", { default: false });
const props = defineProps<{ name: string; repo: string | null }>();

const url = computed(() => collaboratorsUrl(props.repo));
</script>

<template>
  <UModal v-model:open="open" title="Invite a teammate" :description="`Share ${name} in three steps.`">
    <template #body>
      <ol class="text-default grid list-decimal gap-2 pl-5 text-sm">
        <li>Add your teammate on GitHub</li>
        <li>They sign in with GitHub</li>
        <li>They open this site</li>
      </ol>
    </template>
    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton type="button" color="neutral" variant="outline" label="Close" @click="open = false" />
        <UButton
          v-if="url"
          :to="url"
          target="_blank"
          rel="noopener noreferrer"
          icon="i-hugeicons-user-add-01"
          trailing-icon="i-hugeicons-link-square-02"
          label="Add a teammate"
          data-testid="invite-collaborators-link"
        />
      </div>
    </template>
  </UModal>
</template>
