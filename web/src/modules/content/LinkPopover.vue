<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { Check, ExternalLink, Pencil, Unlink } from "@lucide/vue";
import { Input } from "@/components/ui/input";

/**
 * Floating link card for the rich editor. Viewing shows the link under the cursor with Edit and
 * Remove; editing asks for the URL, and for the text too when there is nothing to link yet.
 * Buttons keep the editor's focus and selection by preventing mousedown.
 */
const props = defineProps<{ href?: string; editing: boolean; askText: boolean }>();
const emit = defineEmits<{ apply: [link: { href: string; text: string }]; remove: []; edit: []; cancel: [] }>();

const root = ref<HTMLElement | null>(null);
const url = ref("");
const text = ref("");
const canApply = computed(() => !!url.value.trim());

async function start() {
  url.value = props.href ?? ""; text.value = "";
  await nextTick();
  root.value?.querySelector("input")?.focus();
}
onMounted(() => { if (props.editing) void start(); });
watch(() => props.editing, editing => { if (editing) void start(); });

function apply() { if (canApply.value) emit("apply", { href: url.value.trim(), text: text.value.trim() }); }

defineExpose({ root });
</script>

<template>
  <div ref="root" data-testid="link-popover" class="w-80 rounded-lg border bg-popover p-1 text-sm text-popover-foreground shadow-lg">
    <form v-if="editing" class="grid gap-1 p-1" @submit.prevent="apply" @keydown.esc.prevent="emit('cancel')">
      <div class="flex items-center gap-1">
        <Input v-model="url" data-testid="link-url" placeholder="Paste a URL, /path, or #heading" class="h-8" />
        <button type="submit" title="Apply" aria-label="Apply" data-testid="link-apply" :disabled="!canApply" class="flex size-8 shrink-0 items-center justify-center rounded-md hover:bg-accent disabled:opacity-40"><Check class="size-4" /></button>
      </div>
      <Input v-if="askText" v-model="text" data-testid="link-text" placeholder="Link text (defaults to the URL)" class="h-8" />
    </form>
    <div v-else class="flex items-center gap-0.5">
      <a :href="href" target="_blank" rel="noopener noreferrer" data-testid="link-open" class="flex min-w-0 flex-1 items-center gap-1.5 rounded-md px-2 py-1 text-muted-foreground hover:bg-accent hover:text-foreground">
        <ExternalLink class="size-3.5 shrink-0" /><span class="truncate">{{ href }}</span>
      </a>
      <button type="button" title="Edit link" aria-label="Edit link" data-testid="link-edit" class="flex size-7 shrink-0 items-center justify-center rounded-md hover:bg-accent" @mousedown.prevent @click="emit('edit')"><Pencil class="size-3.5" /></button>
      <button type="button" title="Remove link" aria-label="Remove link" data-testid="link-remove" class="flex size-7 shrink-0 items-center justify-center rounded-md hover:bg-accent" @mousedown.prevent @click="emit('remove')"><Unlink class="size-3.5" /></button>
    </div>
  </div>
</template>
