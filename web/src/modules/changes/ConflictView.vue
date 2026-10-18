<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Button } from "@/components/ui/button";
import type { ConflictChoice, ConflictPreviews, ConflictPreviewState } from "@/core/types";
import { useDaemon } from "@/core/daemon";
import { sectionIdForFile } from "@/modules/preview/conflictHighlight";
import ConflictFile from "./ConflictFile.vue";
import { useChangesStore } from "./store";

const props = defineProps<{ projectId: string }>();
const store = useChangesStore();
const daemon = useDaemon();
const selectedPath = ref<string | null>(null);
const confirmAbandon = ref(false);
const busy = ref(false);
const previews = ref<ConflictPreviews | null>(null);
const previewLoading = ref(false);
const previewError = ref<string | null>(null);
const frameErrors = ref<Partial<Record<"yours" | "latest" | "result", string>>>({});
const resultFrameReady = ref(false);
const resultFrameSettled = ref(false);
const textFallbackAvailable = ref(false);
const textOnly = ref(false);
const viewport = ref<"desktop" | "mobile">("desktop");
const route = ref("/");
let previewRun = 0;
const session = computed(() => store.currentConflict);
const file = computed(() => session.value?.files.find((f) => f.path === selectedPath.value) ?? session.value?.files[0]);

watch(session, (value) => { selectedPath.value = value?.files[0]?.path ?? null; confirmAbandon.value = false; }, { immediate: true });
watch([route, selectedPath, previews], (_value, _old, onCleanup) => {
  frameErrors.value = {};
  resultFrameReady.value = false;
  resultFrameSettled.value = false;
  textFallbackAvailable.value = false;
  textOnly.value = false;
  if (previews.value?.result.status === "ready") {
    const timeout = setTimeout(() => {
      if (!resultFrameSettled.value) {
        frameErrors.value = { ...frameErrors.value, result: "The visual result did not load. You can continue using the text comparison." };
        textFallbackAvailable.value = true;
      }
    }, 8_000);
    onCleanup(() => clearTimeout(timeout));
  }
});
watch(() => [session.value?.id, store.candidate?.tree] as const, async ([id]) => {
  const run = ++previewRun;
  previews.value = null;
  previewError.value = null;
  if (!id) return;
  previewLoading.value = true;
  try { const next = await daemon.api.conflictPreviews(props.projectId, id); if (run === previewRun) previews.value = next; }
  catch (e) { if (run === previewRun) previewError.value = e instanceof Error ? e.message : String(e); }
  finally { if (run === previewRun) previewLoading.value = false; }
}, { immediate: true });

function frameUrl(state: ConflictPreviewState): string {
  if (state.status !== "ready") return "";
  const url = new URL(state.url, daemon.config.value.origin);
  const cleanRoute = route.value.trim().replace(/^\/+/, "");
  url.pathname = `${url.pathname}${cleanRoute}`;
  const section = file.value && sectionIdForFile(file.value.path);
  if (section) { url.searchParams.set("section", section); url.hash = section; }
  return url.toString();
}

function frameLoaded(event: Event, source: "yours" | "latest" | "result") {
  const doc = (event.target as HTMLIFrameElement).contentDocument;
  const missing = doc?.contentType === "application/json" || doc?.body?.textContent?.includes('"code":"ENOENT"');
  if (source === "result") {
    resultFrameSettled.value = true;
    resultFrameReady.value = !!doc && !missing;
    textFallbackAvailable.value = !doc;
    if (missing) textOnly.value = false;
  }
  frameErrors.value = {
    ...frameErrors.value,
    [source]: missing ? "This page is missing from the preview. Choose another page above."
      : !doc ? "The visual page did not load. You can continue using the text comparison." : undefined,
  };
}

async function choose(choice: ConflictChoice) {
  if (!file.value) return;
  busy.value = true;
  try { await store.chooseConflict(props.projectId, file.value.path, choice); }
  catch { /* Error shown from store. */ }
  finally { busy.value = false; }
}

async function finish() {
  busy.value = true;
  try { await store.finishConflict(props.projectId); }
  catch { /* Error shown from store. */ }
  finally { busy.value = false; }
}

async function abandon() {
  busy.value = true;
  try { await store.abandonConflict(props.projectId); }
  catch { /* Error shown from store. */ }
  finally { busy.value = false; confirmAbandon.value = false; }
}
</script>

<template>
  <div class="bg-background absolute inset-0 z-20 flex min-h-0 flex-col" data-testid="conflict-view">
    <header class="border-b px-4 py-3">
      <div class="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 class="text-lg font-semibold">Resolve changes</h2>
          <p class="text-muted-foreground text-sm">Your change and the latest main edited the same file. Review each choice before finishing.</p>
        </div>
        <div class="flex gap-2">
          <Button v-if="!confirmAbandon" variant="outline" size="sm" data-testid="abandon-conflict" @click="confirmAbandon = true">Abandon</Button>
          <Button v-else variant="destructive" size="sm" data-testid="confirm-abandon" :disabled="busy" @click="abandon">Confirm abandon</Button>
          <Button v-if="confirmAbandon" variant="ghost" size="sm" @click="confirmAbandon = false">Cancel</Button>
          <Button size="sm" data-testid="finish-conflict" :disabled="busy || previewLoading || !store.candidate?.complete || previews?.result.status === 'invalid' || (previews?.result.status === 'ready' && !resultFrameReady && !textOnly)" @click="finish">Finish update</Button>
        </div>
      </div>
      <p v-if="store.conflictError" class="text-destructive mt-2 text-sm" role="alert">{{ store.conflictError }}</p>
    </header>
    <div v-if="session" class="flex min-h-0 flex-1">
      <aside class="border-r w-60 shrink-0 overflow-auto p-3">
        <p class="text-muted-foreground mb-2 text-xs">{{ session.branch }}</p>
        <button v-for="item in session.files" :key="item.path" type="button" class="hover:bg-background hover:btn-raised-soft dark:hover:bg-accent/50 block w-full rounded px-2 py-2 text-left text-sm transition-all active:translate-y-px" :class="item.path === file?.path ? 'bg-background btn-raised-soft dark:bg-accent/50' : ''" @click="selectedPath = item.path">
          <span class="block truncate">{{ item.path.split('/').at(-1) }}</span>
          <span class="text-muted-foreground block truncate text-xs">{{ item.path }}</span>
          <span v-if="session.choices[item.path]" class="text-xs">{{ session.choices[item.path] }}</span>
        </button>
      </aside>
      <div class="min-w-0 flex-1 overflow-auto p-4">
        <ConflictFile v-if="file" :file="file" :choice="session.choices[file.path]" @choose="choose" />
        <section class="mt-6 border-t pt-4" data-testid="conflict-previews">
          <div class="mb-3 flex flex-wrap items-center gap-2">
            <h3 class="mr-auto font-semibold">Visual comparison</h3>
            <label class="text-sm">Page <input v-model="route" aria-label="Preview page" class="border-input bg-background ml-1 w-40 rounded border px-2 py-1" placeholder="/" /></label>
            <Button size="sm" :variant="viewport === 'desktop' ? 'default' : 'outline'" @click="viewport = 'desktop'">Desktop</Button>
            <Button size="sm" :variant="viewport === 'mobile' ? 'default' : 'outline'" @click="viewport = 'mobile'">Mobile</Button>
          </div>
          <p v-if="previewLoading" class="text-muted-foreground text-sm">Building visual previews…</p>
          <p v-if="previewError" class="text-amber-700 text-sm" role="status">Visual previews unavailable: {{ previewError }}. You can still compare the text and choose a resolution.</p>
          <p v-if="file && !sectionIdForFile(file.path)" class="text-muted-foreground mb-2 text-xs">This file may affect several sections. Compare the full page and switch pages as needed.</p>
          <div v-if="previews" class="grid gap-3 xl:grid-cols-3">
            <div v-for="source in (['yours', 'latest', 'result'] as const)" :key="source" class="min-w-0">
              <h4 class="mb-1 text-sm font-medium">{{ source === 'yours' ? 'Your version' : source === 'latest' ? 'Latest from main' : 'Result' }}</h4>
              <div v-if="previews[source].status === 'ready'" class="mx-auto h-96 max-w-full overflow-hidden rounded border" :style="{ width: viewport === 'mobile' ? '390px' : '100%' }">
                <iframe :key="frameUrl(previews[source])" :src="frameUrl(previews[source])" :title="`${source} preview`" class="h-full w-full border-0" :loading="source === 'result' ? 'eager' : 'lazy'" sandbox="allow-same-origin" referrerpolicy="no-referrer" @load="frameLoaded($event, source)" />
              </div>
              <p v-else class="text-muted-foreground rounded border p-3 text-sm" :class="previews[source].status === 'invalid' ? 'text-destructive' : ''">{{ previews[source].status === 'invalid' ? previews[source].errors : previews[source].reason }}</p>
              <p v-if="frameErrors[source]" class="text-destructive mt-1 text-sm" role="status">{{ frameErrors[source] }}</p>
              <Button v-if="source === 'result' && textFallbackAvailable" size="sm" variant="outline" class="mt-2" @click="textOnly = true">Continue with text comparison</Button>
            </div>
          </div>
        </section>
      </div>
    </div>
    <p v-else class="p-4 text-sm">Loading conflict…</p>
  </div>
</template>
