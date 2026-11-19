<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UPopover from "@nuxt/ui/components/Popover.vue";
import UTextarea from "@nuxt/ui/components/Textarea.vue";
import type { BrowserFramePick } from "@/components/ui/browser-frame";
import { DEVICE_WIDTH } from "@/modules/preview/store";
import { CaptureUnavailable, TabCapture } from "./capture";
import { newShotId, useShotQueue } from "./queue";

/**
 * Turns an area picked on the Review page's frames into a queued screenshot comment: the tab is
 * shared once (the browser asks when picking starts), the area is cut out of it, and a note is
 * written in a popover at the selection. Without screen sharing a note is still possible.
 */
const props = defineProps<{ projectId: string; number: number; picking: string | null; picked: BrowserFramePick | null }>();
const emit = defineEmits<{ cancel: []; done: [] }>();

const queue = useShotQueue(
  () => props.projectId,
  () => props.number,
);
const capture = new TabCapture();
onBeforeUnmount(() => capture.stop());

/** Why there's no screenshot, and whether notes go on without one. */
const unavailable = ref<string | null>(null);
const notesOnly = ref(false);

// Asked from the Pick click, so the browser accepts the request to share the tab.
watch(
  () => props.picking,
  async (side) => {
    if (!side || notesOnly.value || capture.active) return;
    try {
      await capture.start();
      unavailable.value = null;
    } catch (e) {
      unavailable.value = e instanceof CaptureUnavailable ? e.message : "Couldn't share this tab.";
    }
  },
);

const draft = ref<{ area: BrowserFramePick; image: string | null } | null>(null);
const note = ref("");
const busy = ref(false);

watch(
  () => props.picked,
  async (area) => {
    if (!area) return;
    note.value = "";
    let image: string | null = null;
    if (!notesOnly.value && !unavailable.value) {
      busy.value = true;
      try {
        // Two frames: the dimming overlay is gone from the screen before the tab is grabbed.
        await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
        image = (await capture.grab(area.rect)).dataUrl;
      } catch (e) {
        unavailable.value = e instanceof CaptureUnavailable ? e.message : "Couldn't take the screenshot.";
      } finally {
        busy.value = false;
      }
    }
    draft.value = { area, image };
  },
);

/** The popover sits at the selection. */
const reference = computed(() => {
  const r = draft.value?.area.rect;
  return r ? { getBoundingClientRect: () => DOMRect.fromRect(r) } : undefined;
});

const viewport = (a: BrowserFramePick) =>
  DEVICE_WIDTH[a.device] ? `${a.device} ${DEVICE_WIDTH[a.device]}px` : a.device;

function add() {
  const d = draft.value;
  if (!d || (!note.value.trim() && !d.image)) return;
  queue.add({
    id: newShotId(),
    side: d.area.side === "live" ? "live" : "change",
    path: d.area.path,
    viewport: viewport(d.area),
    rect: d.area.rect,
    imageDataUrl: d.image,
    note: note.value.trim(),
    createdAt: new Date().toISOString(),
  });
  close();
}
function close() {
  draft.value = null;
  emit("done");
}
function onKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
    e.preventDefault();
    add();
  }
}
function withoutScreenshots() {
  notesOnly.value = true;
  unavailable.value = null;
}
</script>

<template>
  <div class="pointer-events-none absolute inset-x-3 top-2 z-20 flex justify-center">
    <UAlert
      v-if="picking && unavailable && !notesOnly"
      color="warning"
      variant="subtle"
      icon="i-hugeicons-alert-circle"
      :description="`${unavailable} Screenshots need permission to share this tab.`"
      class="pointer-events-auto max-w-lg shadow-md"
      data-testid="shot-unavailable"
    >
      <template #actions>
        <UButton
          size="xs"
          color="neutral"
          variant="outline"
          label="Add a note without a screenshot"
          data-testid="shot-notes-only"
          @click="withoutScreenshots"
        />
        <UButton size="xs" color="neutral" variant="ghost" label="Cancel" @click="emit('cancel')" />
      </template>
    </UAlert>
  </div>
  <UPopover
    :open="!!draft"
    :reference="reference"
    :content="{ side: 'bottom', align: 'start', sideOffset: 6 }"
    @update:open="(open: boolean) => !open && close()"
  >
    <template #content>
      <form class="flex w-80 flex-col gap-2 p-3" data-testid="shot-note" @submit.prevent="add">
        <img
          v-if="draft?.image"
          :src="draft.image"
          alt="The picked area"
          class="border-default max-h-40 w-full rounded-md border object-contain"
          data-testid="shot-note-image"
        />
        <p v-else-if="draft" class="text-muted text-xs" data-testid="shot-note-no-image">No screenshot: just the note.</p>
        <UTextarea
          v-model="note"
          :rows="3"
          autoresize
          :maxrows="8"
          autofocus
          placeholder="Add a note…"
          aria-label="Note"
          data-testid="shot-note-input"
          @keydown="onKeydown"
        />
        <div class="flex items-center justify-end gap-2">
          <UButton type="button" size="sm" color="neutral" variant="ghost" label="Cancel" data-testid="shot-note-cancel" @click="close" />
          <UButton
            type="submit"
            size="sm"
            label="Add to queue"
            :disabled="busy || (!note.trim() && !draft?.image)"
            data-testid="shot-note-add"
          />
        </div>
      </form>
    </template>
  </UPopover>
</template>
