<script setup lang="ts">
// App settings → Site builder. The Node download orso uses to install packages
// and preview sites, shown in the same row style as the assistant setup.
import { computed, onMounted, ref } from "vue";
import { CircleAlert, Package, RefreshCw } from "@lucide/vue";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Spinner } from "@/components/ui/spinner";
import ProviderRow from "@/modules/assistant/ProviderRow.vue";
import { useAssistantStore } from "@/modules/assistant/store";
import type { AssistantRow, RowAction } from "@/modules/assistant/rows";

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
  <Card data-testid="site-builder-settings">
    <CardHeader>
      <CardTitle>Site builder</CardTitle>
      <CardDescription>Lets orso build and preview your site.</CardDescription>
      <CardAction>
        <Button
          variant="outline"
          size="sm"
          :disabled="rechecking"
          data-testid="site-builder-recheck"
          @click="recheck"
        >
          <RefreshCw :class="{ 'animate-spin': rechecking }" aria-hidden="true" /> Recheck
        </Button>
      </CardAction>
    </CardHeader>

    <CardContent>
      <p
        v-if="loading"
        class="text-muted-foreground flex items-center gap-2 text-sm"
        data-testid="site-builder-loading"
      >
        <Spinner class="size-3.5" aria-hidden="true" /> Checking your setup…
      </p>

      <Alert
        v-else-if="store.error && !store.snapshot"
        variant="destructive"
        data-testid="site-builder-error"
      >
        <CircleAlert aria-hidden="true" />
        <AlertDescription class="flex flex-col items-start gap-2">
          <p>We couldn't check the site builder just now. Make sure orso is still running, then check again.</p>
          <Button
            variant="outline"
            size="sm"
            :disabled="rechecking"
            data-testid="site-builder-retry"
            @click="recheck"
          >
            <RefreshCw :class="{ 'animate-spin': rechecking }" aria-hidden="true" /> Check again
          </Button>
        </AlertDescription>
      </Alert>

      <Empty v-else-if="store.node == null" data-testid="site-builder-not-needed">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <Package aria-hidden="true" />
          </EmptyMedia>
          <EmptyTitle>No separate download</EmptyTitle>
          <EmptyDescription>
            This version of orso doesn't need a separate site builder download.
          </EmptyDescription>
        </EmptyHeader>
      </Empty>

      <ul v-else class="flex flex-col divide-y" data-testid="site-builder-row">
        <ProviderRow :row="row" @action="onAction" />
      </ul>
    </CardContent>
  </Card>
</template>
