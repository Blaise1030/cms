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
import type { AvatarProps, ModalProps } from "@nuxt/ui";
import UButton from "@nuxt/ui/components/Button.vue";
import UFileUpload from "@nuxt/ui/components/FileUpload.vue";
import UFormField from "@nuxt/ui/components/FormField.vue";
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
function onChosen(value: unknown) { choose(value instanceof File ? value : null); }
/**
 * The chosen file as a UFileUpload item, previewed from our own object URL so the dialog owns
 * (and revokes) the only one. A refused file shows the file icon instead of a preview.
 */
const chosenItem = computed(() => chosen.value && {
  name: chosen.value.name,
  size: chosen.value.size,
  avatar: preview.value ? { src: preview.value, alt: "", "data-testid": "image-preview" } as AvatarProps : undefined,
});
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
        <UFileUpload
          reset
          layout="grid"
          icon="i-hugeicons-image-add-01"
          label="Choose an image or drop it here"
          :accept="ACCEPTED_IMAGES"
          :model-value="chosenItem"
          :ui="{ base: 'h-56', files: 'size-full', file: 'size-full', fileLeadingAvatar: 'rounded-md bg-muted/40 [&>img]:object-contain' }"
          data-testid="image-file"
          @update:model-value="onChosen"
        />
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
