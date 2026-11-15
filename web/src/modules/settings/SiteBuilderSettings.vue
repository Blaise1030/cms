<script setup lang="ts">
// App settings → Site builder. The Node download orso uses to install packages
// and preview sites, shown in the same row style as the assistant setup.
import { computed, onMounted, ref } from "vue";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UEmpty from "@nuxt/ui/components/Empty.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import ProviderRow from "@/modules/assistant/ProviderRow.vue";
import { useAssistantStore } from "@/modules/assistant/store";
import type { AssistantRow, RowAction } from "@/modules/assistant/rows";
import SettingsSection from "./SettingsSection.vue";

const store = useAssistantStore();

const rechecking = ref(false);
const loading = computed(() => !store.snapshot && !store.error);
const row = computed<AssistantRow>(() => store.siteBuilder);

onMounted(() => {
  if (!store.snapshot) void store.load();
});

async function recheck() {
  rechecking.value = true;
  try {
    await store.load();
  } finally {
    rechecking.value = false;
  }
}

function onAction(action: RowAction) {
  switch (action) {
    case "download":
    case "update":
      void store.download(row.value.id);
      break;
    case "retry":
      void store.retry(row.value.id);
      break;
    case "cancel":
      void store.cancel(row.value.id);
      break;
    case "signIn":
    case "connect":
    case "none":
      break;
  }
}
</script>

<template>
  <SettingsSection label="Package" icon="i-hugeicons-package" data-testid="site-builder-settings">
    <template #actions>
      <UButton
        color="neutral"
        variant="outline"
        size="sm"
        icon="i-hugeicons-refresh"
        loading-icon="i-hugeicons-refresh"
        label="Recheck"
        :loading="rechecking"
        :disabled="rechecking"
        data-testid="site-builder-recheck"
        @click="recheck"
      />
    </template>

    <p
      v-if="loading"
      class="text-muted flex items-center gap-2 p-4 text-sm"
      data-testid="site-builder-loading"
    >
      <UIcon name="i-hugeicons-loading-03" class="size-3.5 animate-spin" /> Checking your setup…
    </p>

    <div v-else-if="store.error && !store.snapshot" class="p-4">
      <UAlert
        color="error"
        variant="subtle"
        icon="i-hugeicons-alert-circle"
        description="We couldn't check the site builder just now. Make sure orso is still running, then check again."
        data-testid="site-builder-error"
      >
        <template #actions>
          <UButton
            color="neutral"
            variant="outline"
            size="sm"
            icon="i-hugeicons-refresh"
            loading-icon="i-hugeicons-refresh"
            label="Check again"
            :loading="rechecking"
            :disabled="rechecking"
            data-testid="site-builder-retry"
            @click="recheck"
          />
        </template>
      </UAlert>
    </div>

    <UEmpty
      v-else-if="store.node == null"
      variant="naked"
      icon="i-hugeicons-package"
      title="No separate download"
      description="This version of orso doesn't need a separate site builder download."
      data-testid="site-builder-not-needed"
    />

    <ul v-else class="divide-y divide-default [&>li]:px-4" data-testid="site-builder-row">
      <ProviderRow :row="row" @action="onAction" />
    </ul>
  </SettingsSection>
</template>
