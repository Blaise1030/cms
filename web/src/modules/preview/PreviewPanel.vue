<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Monitor, RefreshCw, Smartphone, Tablet } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { isCrossSite } from "@/core/config";
import { useDaemon } from "@/core/daemon";
import type { DevserverState } from "@/core/types";
import { DEVICE_WIDTH, usePreviewStore, type Device } from "./store";

const props = defineProps<{ projectId: string; devserver?: DevserverState }>();

const store = usePreviewStore();
const daemon = useDaemon();

watch(
  () => [props.projectId, props.devserver] as const,
  ([id, state]) => {
    if (state) store.seed(id, state);
  },
  { immediate: true },
);

const state = computed<DevserverState>(
  () => store.status[props.projectId] ?? props.devserver ?? "starting",
);
const src = computed(() => daemon.api.previewUrl(props.projectId));
// A cross-site daemon cannot authenticate the framed preview (SameSite=Lax `wb_session`); see
// `previewUrl` in core/api.ts. Explain instead of showing a wall of 401s; the user may override
// (e.g. sibling subdomains are same-site but look cross-site to us).
const forceFrame = ref(false);
const daemonOrigin = computed(() => daemon.config.value.origin);
const crossSite = computed(
  () => !forceFrame.value && isCrossSite(window.location.origin, daemonOrigin.value),
);
const width = computed(() => DEVICE_WIDTH[store.device]);

const DOT: Record<DevserverState, string> = {
  starting: "bg-amber-500 animate-pulse",
  ready: "bg-emerald-500",
  crashed: "bg-red-500",
};
const LABEL: Record<DevserverState, string> = {
  starting: "Dev server starting",
  ready: "Dev server ready",
  crashed: "Dev server crashed",
};
</script>

<template>
  <div class="flex h-full min-h-0 flex-col" data-testid="preview-panel">
    <div class="flex h-10 shrink-0 items-center justify-between gap-2 border-b px-3">
      <Tabs :model-value="store.device" @update:model-value="(v) => store.setDevice(v as Device)">
        <TabsList>
          <TabsTrigger value="desktop" aria-label="Desktop"><Monitor /></TabsTrigger>
          <TabsTrigger value="tablet" aria-label="Tablet"><Tablet /></TabsTrigger>
          <TabsTrigger value="mobile" aria-label="Mobile"><Smartphone /></TabsTrigger>
        </TabsList>
      </Tabs>
      <div class="flex items-center gap-2">
        <TooltipProvider>
        <Tooltip>
          <TooltipTrigger as-child>
            <span
              class="size-2 rounded-full"
              :class="DOT[state]"
              :data-state="state"
              role="status"
              :aria-label="LABEL[state]"
              data-testid="status-dot"
            />
          </TooltipTrigger>
          <TooltipContent>{{ LABEL[state] }}</TooltipContent>
        </Tooltip>
        </TooltipProvider>
        <Button variant="ghost" size="icon-sm" aria-label="Reload preview" @click="store.reload()">
          <RefreshCw />
        </Button>
      </div>
    </div>

    <div class="bg-muted/30 flex min-h-0 flex-1 justify-center overflow-auto">
      <div
        v-if="state === 'crashed'"
        class="flex flex-col items-center justify-center gap-3 p-6 text-center text-sm"
        role="alert"
      >
        <p>The dev server crashed{{ store.message[projectId] ? `: ${store.message[projectId]}` : "." }}</p>
        <p v-if="store.error" class="text-destructive">{{ store.error }}</p>
        <Button :disabled="store.restarting" @click="store.restart(projectId)">
          {{ store.restarting ? "Restarting…" : "Restart dev server" }}
        </Button>
      </div>
      <div
        v-else-if="state === 'starting'"
        class="text-muted-foreground flex items-center justify-center text-sm"
      >
        Starting dev server…
      </div>
      <div
        v-else-if="crossSite"
        class="flex max-w-md flex-col items-center justify-center gap-3 p-6 text-center text-sm"
        role="note"
        data-testid="preview-cross-site"
      >
        <p class="font-medium">Live preview isn't available from this page</p>
        <p class="text-muted-foreground">
          The daemon at {{ daemonOrigin }} is on a different site than this app, so the
          browser won't send its session cookie to the preview frame. Open the daemon link
          directly (the one printed at startup) to use the live preview.
        </p>
        <Button variant="outline" size="sm" @click="forceFrame = true">Try anyway</Button>
      </div>
      <iframe
        v-else
        :key="`${projectId}:${store.reloadNonce}`"
        :src="src"
        title="Site preview"
        class="h-full border-0 bg-white"
        :style="{ width: width ? `${width}px` : '100%' }"
        data-testid="preview-frame"
      />
    </div>
  </div>
</template>
