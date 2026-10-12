<script setup lang="ts">
import type { HTMLAttributes } from "vue"
import { ref } from "vue"
import { X } from "@lucide/vue"
import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"

const props = defineProps<{
  modelValue: string[]
  placeholder?: string
  id?: string
  class?: HTMLAttributes["class"]
}>()
const emit = defineEmits<{ "update:modelValue": [value: string[]] }>()

const draft = ref("")
const input = ref<HTMLInputElement | null>(null)

/** Adds the typed text as tags (commas split it); blanks and duplicates are skipped. */
function commit() {
  const added = draft.value.split(",").map(t => t.trim()).filter(t => t && !props.modelValue.includes(t))
  draft.value = ""
  if (added.length) emit("update:modelValue", [...props.modelValue, ...new Set(added)])
}
function remove(index: number) {
  emit("update:modelValue", props.modelValue.filter((_, i) => i !== index))
}
function onKeydown(event: KeyboardEvent) {
  if (event.key === "Enter" || event.key === ",") {
    event.preventDefault()
    commit()
  } else if (event.key === "Backspace" && !draft.value && props.modelValue.length) {
    remove(props.modelValue.length - 1)
  }
}
</script>

<template>
  <div
    data-slot="tag-input"
    :class="cn(
      'border-input dark:bg-input/30 flex min-h-9 w-full flex-wrap items-center gap-1 rounded-md border bg-transparent px-2 py-1 text-sm shadow-xs transition-[color,box-shadow]',
      'focus-within:border-ring focus-within:ring-ring/50 focus-within:ring-3',
      props.class,
    )"
    @click="input?.focus()"
  >
    <Badge v-for="(tag, index) in modelValue" :key="`${tag}-${index}`" variant="secondary" class="gap-1 pr-1" data-slot="tag">
      {{ tag }}
      <button
        type="button"
        class="text-muted-foreground hover:text-foreground rounded-full outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
        :aria-label="`Remove ${tag}`"
        data-slot="tag-remove"
        @click.stop="remove(index)"
      >
        <X class="size-3" />
      </button>
    </Badge>
    <input
      :id="id"
      ref="input"
      v-model="draft"
      type="text"
      class="placeholder:text-muted-foreground min-w-20 flex-1 bg-transparent py-0.5 outline-none"
      :placeholder="modelValue.length ? undefined : placeholder"
      @keydown="onKeydown"
      @blur="commit"
    >
  </div>
</template>
