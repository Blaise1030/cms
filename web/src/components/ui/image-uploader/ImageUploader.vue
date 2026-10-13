<script setup lang="ts">
import type { HTMLAttributes } from "vue"
import { ref, watch } from "vue"
import { ImageOff, ImageUp, RefreshCw, Trash2 } from "@lucide/vue"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

const props = withDefaults(defineProps<{
  /** URL to preview; empty shows the drop zone. */
  src?: string | null
  accept?: string
  disabled?: boolean
  class?: HTMLAttributes["class"]
}>(), { accept: "image/*" })
const emit = defineEmits<{ select: [file: File], remove: [] }>()

const picker = ref<HTMLInputElement | null>(null)
const dragging = ref(false)
const broken = ref(false)
watch(() => props.src, () => { broken.value = false })

function pick() { if (!props.disabled) picker.value?.click() }
function onPick(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ""
  if (file) emit("select", file)
}
function onDrop(event: DragEvent) {
  dragging.value = false
  const file = event.dataTransfer?.files?.[0]
  if (file && !props.disabled) emit("select", file)
}
</script>

<template>
  <div
    data-slot="image-uploader"
    :class="cn(
      'group relative flex aspect-video w-full items-center justify-center overflow-hidden rounded-md border text-sm transition-colors',
      src ? 'bg-muted/30' : 'border-dashed hover:bg-accent/40 cursor-pointer',
      dragging && 'border-ring bg-accent/50',
      disabled && 'pointer-events-none opacity-50',
      props.class,
    )"
    :role="src ? undefined : 'button'"
    :tabindex="src ? undefined : 0"
    @click="!src && pick()"
    @keydown.enter.prevent="!src && pick()"
    @keydown.space.prevent="!src && pick()"
    @dragover.prevent="dragging = true"
    @dragleave="dragging = false"
    @drop.prevent="onDrop"
  >
    <input ref="picker" type="file" class="hidden" :accept="accept" data-slot="image-uploader-input" @change="onPick">

    <template v-if="src">
      <img v-if="!broken" :src="src" alt="" class="size-full object-contain" data-slot="image-uploader-preview" @error="broken = true">
      <div v-else class="flex flex-col items-center gap-1 text-muted-foreground" data-slot="image-uploader-broken">
        <ImageOff class="size-6" />
        <span class="text-xs">Preview unavailable</span>
      </div>
      <div class="absolute inset-x-0 bottom-0 flex justify-end gap-1 bg-gradient-to-t from-black/60 to-transparent p-2 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
        <Button type="button" size="sm" variant="secondary" data-slot="image-uploader-replace" @click.stop="pick">
          <RefreshCw /> Replace
        </Button>
        <Button type="button" size="icon" variant="secondary" class="size-8" aria-label="Remove image" data-slot="image-uploader-remove" @click.stop="emit('remove')">
          <Trash2 />
        </Button>
      </div>
    </template>

    <div v-else class="flex flex-col items-center gap-1.5 px-4 text-center text-muted-foreground">
      <ImageUp class="size-6" />
      <span><span class="text-foreground font-medium">Click to upload</span> or drag an image here</span>
      <slot name="hint" />
    </div>
  </div>
</template>
