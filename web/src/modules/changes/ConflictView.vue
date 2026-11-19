<script setup lang="ts">
import { computed, ref, watch } from "vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import UInput from "@nuxt/ui/components/Input.vue";
import USplitter from "@nuxt/ui/components/Splitter.vue";
import type { SplitterItem } from "@nuxt/ui";
import type { ConflictChoice, ConflictPreviews, ConflictPreviewState } from "@/core/types";
import { useDaemon } from "@/core/daemon";
import { sectionIdForFile } from "@/modules/preview/conflictHighlight";
import ConflictFile from "./ConflictFile.vue";
import { CHOICE_LABEL, conflictLabel, conflictSubject } from "./conflictLabels";
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

const SPLIT: SplitterItem[] = [
  { id: "conflict-list", slot: "list", defaultSize: 20, minSize: 12, maxSize: 40 },
  { id: "conflict-detail", slot: "detail", defaultSize: 80, minSize: 50 },
];
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
        frameErrors.value = { ...frameErrors.value, result: "The preview didn't load. You can continue without it." };
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
      : !doc ? "The preview didn't load. You can continue without it." : undefined,
  };
}

const decided = computed(() => session.value?.files.filter((f) => session.value?.choices[f.path]).length ?? 0);
const choice = computed(() => (file.value ? session.value?.choices[file.value.path] : undefined));
const canKeepBoth = computed(() => !!file.value && !file.value.binary && !file.value.oursDeleted && !file.value.theirsDeleted);

/** The two versions a writer picks between, each shown as a selectable preview card. */
const cards = computed(() => [
  {
    source: "yours" as const,
    choice: "yours" as const,
    title: "Your version",
    label: file.value?.oursDeleted ? "Keep your deletion" : "Keep your version",
    testid: "keep-yours",
  },
  {
    source: "latest" as const,
    choice: "latest" as const,
    title: "Live site version",
    label: file.value?.theirsDeleted ? "Use the live site's deletion" : "Use the live site's version",
    testid: "use-latest",
  },
]);

/** Why one side is missing, in plain words. */
const deletionNote = computed(() => {
  if (file.value?.oursDeleted) return "You removed this, but it was changed on the live site since.";
  if (file.value?.theirsDeleted) return "This was removed from the live site, but you changed it.";
  return null;
});

const finishBlocked = computed(
  () => busy.value || previewLoading.value || !store.candidate?.complete || previews.value?.result.status === "invalid"
    || (previews.value?.result.status === "ready" && !resultFrameReady.value && !textOnly.value),
);
/** Shown on hover over a disabled Save, so it always says why. */
const finishReason = computed(() => {
  if (!finishBlocked.value || busy.value) return undefined;
  if (!store.candidate?.complete) return "Choose a version for every part first.";
  if (previews.value?.result.status === "invalid") return "The site doesn't work with these choices. Try a different version.";
  return "Waiting for the preview of the result to load.";
});

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
    <header class="px-4 py-3">
      <div class="flex flex-wrap items-center justify-between gap-3">
        <div class="min-w-0">
          <h2 class="text-highlighted text-lg font-semibold">Resolve changes</h2>
          <p class="text-muted text-sm">
            <template v-if="session">You and a teammate both changed {{ conflictSubject(session.files.map((f) => f.path)) }}. Choose which version to keep.</template>
            <template v-else>Loading…</template>
          </p>
        </div>
        <div class="flex items-center gap-2">
          <template v-if="!confirmAbandon">
            <UButton color="neutral" variant="ghost" size="sm" label="Cancel" data-testid="abandon-conflict" @click="confirmAbandon = true" />
          </template>
          <template v-else>
            <span class="text-muted text-sm">Stop and keep your change as it is?</span>
            <UButton color="neutral" variant="ghost" size="sm" label="Go back" @click="confirmAbandon = false" />
            <UButton color="error" variant="soft" size="sm" label="Yes, stop" data-testid="confirm-abandon" :disabled="busy" @click="abandon" />
          </template>
          <UButton size="sm" label="Save and continue" data-testid="finish-conflict" :disabled="finishBlocked" :title="finishReason" @click="finish" />
        </div>
      </div>
      <p v-if="store.conflictError" class="text-error mt-2 text-sm" role="alert">{{ store.conflictError }}</p>
    </header>

    <USplitter
      v-if="session"
      :items="SPLIT"
      auto-save-id="conflict-sidebar"
      class="min-h-0 flex-1"
      :ui="{ handle: 'flex items-center justify-center' }"
    >
      <template #resize-handle>
        <div
          class="bg-accented group-hover:bg-inverted/70 group-data-[resize-handle-state=drag]:bg-inverted h-8 w-1 rounded-full transition-colors"
          aria-hidden="true"
        />
      </template>
      <template #list>
      <aside class="flex h-full w-full flex-col gap-1 overflow-auto p-3">
        <p class="text-muted mb-1 px-2 text-xs" data-testid="conflict-progress">{{ decided }} of {{ session.files.length }} decided</p>
        <UButton
          v-for="item in session.files"
          :key="item.path"
          color="neutral"
          :variant="item.path === file?.path ? 'soft' : 'ghost'"
          block
          class="items-start justify-start text-left font-normal"
          :aria-current="item.path === file?.path ? 'true' : undefined"
          data-testid="conflict-item"
          @click="selectedPath = item.path"
        >
          <UIcon
            :name="session.choices[item.path] ? 'i-hugeicons-checkmark-circle-02' : 'i-hugeicons-record'"
            class="mt-0.5 size-4 shrink-0"
            :class="session.choices[item.path] ? 'text-success' : 'text-warning'"
          />
          <span class="flex min-w-0 flex-col">
            <span class="text-default truncate text-sm">{{ conflictLabel(item.path) }}</span>
            <span class="text-muted truncate text-xs">{{ session.choices[item.path] ? CHOICE_LABEL[session.choices[item.path]!] : "Needs a decision" }}</span>
          </span>
        </UButton>
      </aside>
      </template>

      <template #detail>
      <div v-if="file" class="h-full min-w-0 w-full space-y-4 overflow-auto p-4" data-testid="conflict-file">
        <div class="flex flex-wrap items-center gap-2">
          <h3 class="text-highlighted mr-auto font-semibold">{{ conflictLabel(file.path) }}</h3>
          <UInput v-model="route" size="sm" aria-label="Preview page" class="w-36" placeholder="/" icon="i-hugeicons-link-01" title="Page to preview" />
          <UButton size="sm" color="neutral" :variant="viewport === 'desktop' ? 'soft' : 'ghost'" icon="i-hugeicons-computer" label="Desktop" @click="viewport = 'desktop'" />
          <UButton size="sm" color="neutral" :variant="viewport === 'mobile' ? 'soft' : 'ghost'" icon="i-hugeicons-smart-phone-01" label="Mobile" @click="viewport = 'mobile'" />
        </div>
        <p v-if="deletionNote" class="text-muted text-sm">{{ deletionNote }}</p>
        <p v-if="previewError" class="text-warning text-sm" role="status">Previews aren't available right now. You can still choose a version, or check the code below.</p>
        <p v-else-if="!sectionIdForFile(file.path)" class="text-muted text-xs">This may change more than one part of the page. Switch pages above to check.</p>

        <UButton
          :color="choice === 'both' ? 'primary' : 'neutral'"
          variant="soft"
          block
          class="justify-start text-left"
          :leading-icon="choice === 'both' ? 'i-hugeicons-checkmark-circle-02' : 'i-hugeicons-circle'"
          :ui="{ leadingIcon: choice === 'both' ? 'text-primary' : 'text-dimmed' }"
          :aria-pressed="choice === 'both'"
          :disabled="busy || !canKeepBoth"
          data-testid="keep-both"
          @click="choose('both')"
        >
          Keep both
          <span class="text-muted font-normal">· your text first, then the live site's</span>
        </UButton>

        <div class="grid gap-3 lg:grid-cols-2" data-testid="conflict-previews">
          <div
            v-for="card in cards"
            :key="card.source"
            class="flex min-w-0 flex-col overflow-hidden rounded-lg transition-colors"
            :class="choice === card.choice ? 'bg-primary/10' : 'bg-elevated/50'"
          >
            <UButton
              :color="choice === card.choice ? 'primary' : 'neutral'"
              variant="ghost"
              block
              class="justify-start rounded-none text-left"
              :leading-icon="choice === card.choice ? 'i-hugeicons-checkmark-circle-02' : 'i-hugeicons-circle'"
              :ui="{ leadingIcon: choice === card.choice ? 'text-primary' : 'text-dimmed' }"
              :label="card.label"
              :aria-pressed="choice === card.choice"
              :disabled="busy"
              :data-testid="card.testid"
              @click="choose(card.choice)"
            />
            <div class="bg-muted flex flex-1 justify-center">
              <div v-if="previews?.[card.source].status === 'ready'" class="h-64 max-w-full" :style="{ width: viewport === 'mobile' ? '390px' : '100%' }">
                <iframe :key="frameUrl(previews[card.source])" :src="frameUrl(previews[card.source])" :title="`${card.source} preview`" class="bg-default h-full w-full border-0" loading="lazy" sandbox="allow-same-origin" referrerpolicy="no-referrer" @load="frameLoaded($event, card.source)" />
              </div>
              <p v-else-if="previewLoading" class="text-muted self-center p-6 text-sm">Building preview…</p>
              <p v-else-if="previews" class="text-muted self-center p-6 text-sm">{{ previews[card.source].status === 'invalid' ? "This version doesn't build." : "No preview for this version." }}</p>
              <p v-else class="text-muted self-center p-6 text-sm">{{ card.title }}</p>
            </div>
            <p v-if="frameErrors[card.source]" class="text-error px-3 py-2 text-sm" role="status">{{ frameErrors[card.source] }}</p>
          </div>
        </div>

        <section v-if="previews" class="space-y-2" data-testid="conflict-result">
          <h4 class="text-highlighted text-sm font-medium">After you save</h4>
          <div v-if="previews.result.status === 'ready'" class="bg-muted flex justify-center overflow-hidden rounded-lg">
            <div class="h-64 max-w-full" :style="{ width: viewport === 'mobile' ? '390px' : '100%' }">
              <iframe :key="frameUrl(previews.result)" :src="frameUrl(previews.result)" title="result preview" class="bg-default h-full w-full border-0" loading="eager" sandbox="allow-same-origin" referrerpolicy="no-referrer" @load="frameLoaded($event, 'result')" />
            </div>
          </div>
          <UAlert v-else :color="previews.result.status === 'invalid' ? 'error' : 'neutral'" variant="soft" :description="previews.result.status === 'invalid' ? previews.result.errors : previews.result.reason" />
          <p v-if="frameErrors.result" class="text-error text-sm" role="status">{{ frameErrors.result }}</p>
          <UButton v-if="textFallbackAvailable" color="neutral" variant="outline" size="sm" label="Continue without the preview" @click="textOnly = true" />
        </section>

        <ConflictFile :file="file" />
      </div>
      </template>
    </USplitter>
    <p v-else class="text-muted p-4 text-sm">Loading…</p>
  </div>
</template>
