<script setup lang="ts">
import { onBeforeUnmount, ref } from "vue";
import { Download, X } from "@lucide/vue";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
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
  <Alert
    v-if="update && !dismissed"
    class="fixed right-4 bottom-4 z-50 w-80 pr-9 shadow-md"
    role="status"
    data-testid="update-notice"
  >
    <Download />
    <AlertTitle>Update ready — restart to apply</AlertTitle>
    <AlertDescription>
      <p>orso {{ update.version }} has been downloaded.</p>
      <Button size="sm" data-testid="update-restart" @click="props.navigate(update.applyUrl)">
        Restart to update
      </Button>
    </AlertDescription>
    <Button
      variant="ghost"
      size="icon"
      class="absolute top-2 right-2 size-6"
      aria-label="Dismiss"
      data-testid="update-dismiss"
      @click="dismissed = true"
    >
      <X />
    </Button>
  </Alert>
</template>
