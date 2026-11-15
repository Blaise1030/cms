<script setup lang="ts">
import { onBeforeUnmount, ref } from "vue";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import { type DesktopUpdate, UPDATE_EVENT, parseUpdate } from "@/app/desktopUpdate";

const props = withDefaults(defineProps<{ navigate?: (url: string) => void }>(), {
  navigate: (url: string) => window.location.assign(url),
});

const update = ref<DesktopUpdate | null>(parseUpdate(window.__ORSO_UPDATE__));
const dismissed = ref(false);

function onReady(e: Event) {
  const next = parseUpdate((e as CustomEvent).detail);
  if (!next) return;
  if (next.version !== update.value?.version) dismissed.value = false;
  update.value = next;
}
window.addEventListener(UPDATE_EVENT, onReady);
onBeforeUnmount(() => window.removeEventListener(UPDATE_EVENT, onReady));
</script>

<template>
  <UAlert
    v-if="update && !dismissed"
    color="neutral"
    variant="outline"
    icon="i-hugeicons-download-01"
    title="Update ready — restart to apply"
    close
    class="bg-default fixed right-4 bottom-4 z-50 w-80 shadow-md"
    role="status"
    data-testid="update-notice"
  >
    <template #description>
      <p>orso {{ update.version }} has been downloaded.</p>
      <UButton size="sm" class="mt-2" data-testid="update-restart" @click="props.navigate(update.applyUrl)">
        Restart to update
      </UButton>
    </template>
    <template #close>
      <UButton
        color="neutral"
        variant="ghost"
        size="xs"
        icon="i-hugeicons-cancel-01"
        aria-label="Dismiss"
        data-testid="update-dismiss"
        @click="dismissed = true"
      />
    </template>
  </UAlert>
</template>
