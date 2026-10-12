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
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { Image as ImageIcon, ImagePlus } from "@lucide/vue";
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
  <Dialog v-model:open="open">
    <DialogContent data-testid="image-dialog">
      <form class="grid gap-4" @submit.prevent="insert">
        <DialogHeader>
          <DialogTitle>Add image</DialogTitle>
          <DialogDescription>PNG, JPEG, WebP, or GIF up to 10 MiB. It is uploaded when you save.</DialogDescription>
        </DialogHeader>
        <label
          class="group relative flex cursor-pointer flex-col overflow-hidden rounded-lg border transition-colors has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-ring"
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
            <div class="flex items-center gap-2 border-t bg-background/60 px-3 py-2 text-xs">
              <ImageIcon class="size-3.5 shrink-0 text-muted-foreground" />
              <span class="min-w-0 flex-1 truncate font-medium">{{ chosen?.name }}</span>
              <span class="shrink-0 text-muted-foreground">{{ chosen ? fileSize(chosen.size) : "" }}</span>
              <span class="shrink-0 text-muted-foreground underline-offset-2 group-hover:text-foreground group-hover:underline">Replace</span>
            </div>
          </template>
          <div v-else class="flex h-40 flex-col items-center justify-center gap-2 text-sm text-muted-foreground">
            <span class="flex size-10 items-center justify-center rounded-full bg-muted"><ImagePlus class="size-5" /></span>
            <span><span class="font-medium text-foreground">Choose an image</span> or drop it here</span>
            <span v-if="chosen" class="max-w-full truncate px-4 text-xs">{{ chosen.name }}</span>
          </div>
        </label>
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
