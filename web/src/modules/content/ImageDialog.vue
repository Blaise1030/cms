<script lang="ts">
import type { InjectionKey } from "vue";

/** An image chosen in the dialog, held in the draft with a local preview until Save. */
export interface PendingImage { clientId: string; file: File; alt: string; filename: string; previewUrl: string }
/** Where editors hand inserted images so the draft can upload them on Save. */
export interface ImageRegistry { add: (image: PendingImage) => void }
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
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";

/**
 * The one preview and alt-text dialog behind paste, drop, and the file picker. `file` is the
 * pasted or dropped file; without one the writer picks a file here. Nothing is inserted until
 * the alt text is non-empty; the preview URL then belongs to the inserted draft image.
 */
const open = defineModel<boolean>("open", { default: false });
const props = defineProps<{ file?: File | null }>();
const emit = defineEmits<{ insert: [image: PendingImage] }>();

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
  <Dialog v-model:open="open">
    <DialogContent data-testid="image-dialog">
      <form class="grid gap-4" @submit.prevent="insert">
        <DialogHeader>
          <DialogTitle>Add image</DialogTitle>
          <DialogDescription>PNG, JPEG, WebP, or GIF up to 10 MiB. It is uploaded when you save.</DialogDescription>
        </DialogHeader>
        <input type="file" accept="image/png,image/jpeg,image/webp,image/gif" data-testid="image-file" class="text-sm" @change="onPick" />
        <img v-if="preview" :src="preview" alt="" data-testid="image-preview" class="max-h-48 w-auto rounded-md border object-contain" />
        <p v-if="error" role="alert" class="text-sm text-destructive" data-testid="image-error">{{ error }}</p>
        <label class="grid gap-1 text-xs font-medium">Alt text (required)
          <Input v-model="alt" data-testid="image-alt" placeholder="Describe the image" />
        </label>
        <DialogFooter>
          <Button type="button" variant="outline" data-testid="image-cancel" @click="open = false">Cancel</Button>
          <Button type="submit" data-testid="image-insert" :disabled="!canInsert">Insert</Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
