<script lang="ts">
import type { InjectionKey } from "vue";

/** An image chosen in the dialog, held in the draft with a local preview until Save. */
export interface PendingImage { clientId: string; file: File; alt: string; filename: string; previewUrl: string }
/** Where editors hand inserted images so the draft can upload them on Save. */
export interface ImageRegistry {
  add: (image: PendingImage) => void;
  /** A URL that displays an image link as written in the entry (entry-relative paths). */
  resolve?: (src: string) => string;
}
export const ACCEPTED_IMAGES = "image/png,image/jpeg,image/webp,image/gif";
export const IMAGE_REGISTRY: InjectionKey<ImageRegistry> = Symbol("content-images");

export const MAX_IMAGE_BYTES = 10 * 1024 * 1024;
const ACCEPTED = ["image/png", "image/jpeg", "image/webp", "image/gif"];

/** Alt text that cannot break out of Markdown image syntax. */
export function escapeAlt(alt: string): string {
  return alt.replace(/\s*\n\s*/g, " ").replace(/[\\[\]]/g, c => `\\${c}`);
}
export function markdownImage(alt: string, url: string): string {
  return `![${escapeAlt(alt)}](${url})`;
}
/** A quick check for the writer; the daemon decides from the bytes. */
function problem(file: File): string | null {
  if (!ACCEPTED.includes(file.type)) return "Choose a PNG, JPEG, WebP, or GIF image.";
  if (file.size > MAX_IMAGE_BYTES) return "Images must be 10 MiB or smaller.";
  return null;
}
function newClientId(): string {
  return globalThis.crypto?.randomUUID?.() ?? `img-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}
</script>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, useId, watch } from "vue";
import type { ModalProps } from "@nuxt/ui";
import UButton from "@nuxt/ui/components/Button.vue";
import UFormField from "@nuxt/ui/components/FormField.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import UInput from "@nuxt/ui/components/Input.vue";
import UModal from "@nuxt/ui/components/Modal.vue";

/**
 * The one preview and alt-text dialog behind paste, drop, and the file picker. `file` is the
 * pasted or dropped file; without one the writer picks a file here. Nothing is inserted until
 * the alt text is non-empty; the preview URL then belongs to the inserted draft image.
 */
const open = defineModel<boolean>("open", { default: false });
const props = defineProps<{ file?: File | null }>();
const emit = defineEmits<{ insert: [image: PendingImage] }>();

/** Nuxt UI passes extra `content` keys to the dialog panel as attributes; this one names it for tests. */
const dialogContent = { "data-testid": "image-dialog" } as ModalProps["content"];
/** The footer's Insert button submits the body's form through this id. */
const formId = useId();
const chosen = ref<File | null>(null);
const preview = ref<string | null>(null);
const alt = ref("");
const error = computed(() => (chosen.value ? problem(chosen.value) : null));
const canInsert = computed(() => !!chosen.value && !error.value && !!alt.value.trim());

function release() { if (preview.value) URL.revokeObjectURL(preview.value); preview.value = null; }
function choose(file: File | null) {
  release();
  chosen.value = file;
  preview.value = file && !problem(file) ? URL.createObjectURL(file) : null;
}
function onPick(event: Event) { choose((event.target as HTMLInputElement).files?.[0] ?? null); }
const dragging = ref(false);
function onDrop(event: DragEvent) {
  dragging.value = false;
  const file = event.dataTransfer?.files?.[0];
  if (file) choose(file);
}
function fileSize(bytes: number) {
  return bytes < 1024 * 1024 ? `${Math.max(1, Math.round(bytes / 1024))} KB` : `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
watch([open, () => props.file], ([isOpen]) => {
  if (isOpen) { alt.value = ""; choose(props.file ?? null); } else release();
}, { immediate: true });
onBeforeUnmount(release);

function insert() {
  const file = chosen.value;
  if (!canInsert.value || !file || !preview.value) return;
  const image: PendingImage = { clientId: newClientId(), file, alt: alt.value.trim(), filename: file.name, previewUrl: preview.value };
  preview.value = null; // handed to the draft; not revoked on close
  emit("insert", image);
  open.value = false;
}
</script>

<template>
  <UModal
    v-model:open="open"
    title="Add image"
    description="PNG, JPEG, WebP, or GIF up to 10 MiB. It is uploaded when you save."
    :content="dialogContent"
  >
    <template #body>
      <form :id="formId" class="grid gap-4" @submit.prevent="insert">
        <label
          class="group relative flex cursor-pointer flex-col overflow-hidden rounded-lg border border-default transition-colors has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-primary"
          :class="[preview ? 'bg-muted/40' : 'border-dashed hover:bg-muted/40', dragging && 'border-primary bg-muted/60']"
          @dragover.prevent="dragging = true"
          @dragleave="dragging = false"
          @drop.prevent="onDrop"
        >
          <input type="file" :accept="ACCEPTED_IMAGES" data-testid="image-file" class="sr-only" @change="onPick" />
          <template v-if="preview">
            <div class="flex h-56 items-center justify-center p-4">
              <img :src="preview" alt="" data-testid="image-preview" class="max-h-full max-w-full rounded object-contain shadow-sm" />
            </div>
            <div class="flex items-center gap-2 border-t border-default bg-default/60 px-3 py-2 text-xs">
              <UIcon name="i-hugeicons-image-01" class="text-muted size-3.5 shrink-0" />
              <span class="text-highlighted min-w-0 flex-1 truncate font-medium">{{ chosen?.name }}</span>
              <span class="text-muted shrink-0">{{ chosen ? fileSize(chosen.size) : "" }}</span>
              <span class="text-muted shrink-0 underline-offset-2 group-hover:text-highlighted group-hover:underline">Replace</span>
            </div>
          </template>
          <div v-else class="text-muted flex h-40 flex-col items-center justify-center gap-2 text-sm">
            <span class="bg-elevated flex size-10 items-center justify-center rounded-full"><UIcon name="i-hugeicons-image-add-01" class="size-5" /></span>
            <span><span class="text-highlighted font-medium">Choose an image</span> or drop it here</span>
            <span v-if="chosen" class="max-w-full truncate px-4 text-xs">{{ chosen.name }}</span>
          </div>
        </label>
        <p v-if="error" role="alert" class="text-error text-sm" data-testid="image-error">{{ error }}</p>
        <UFormField label="Alt text" required>
          <UInput v-model="alt" class="w-full" data-testid="image-alt" placeholder="Describe the image" />
        </UFormField>
      </form>
    </template>
    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton type="button" color="neutral" variant="outline" label="Cancel" data-testid="image-cancel" @click="open = false" />
        <UButton type="submit" :form="formId" label="Insert" data-testid="image-insert" :disabled="!canInsert" />
      </div>
    </template>
  </UModal>
</template>
