<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UInput from "@nuxt/ui/components/Input.vue";

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
  <div ref="root" data-testid="link-popover" class="w-80 rounded-lg border border-default bg-default p-1 text-sm text-default shadow-lg">
    <form v-if="editing" class="grid gap-1 p-1" @submit.prevent="apply" @keydown.esc.prevent="emit('cancel')">
      <div class="flex items-center gap-1">
        <UInput v-model="url" data-testid="link-url" placeholder="Paste a URL, /path, or #heading" size="sm" class="flex-1" />
        <UButton type="submit" color="neutral" variant="ghost" size="sm" square icon="i-hugeicons-tick-02" title="Apply" aria-label="Apply" data-testid="link-apply" :disabled="!canApply" />
      </div>
      <UInput v-if="askText" v-model="text" data-testid="link-text" placeholder="Link text (defaults to the URL)" size="sm" />
    </form>
    <div v-else class="flex items-center gap-0.5">
      <UButton
        :to="href"
        target="_blank"
        rel="noopener noreferrer"
        color="neutral"
        variant="ghost"
        size="xs"
        icon="i-hugeicons-link-square-02"
        :label="href"
        class="text-muted min-w-0 flex-1"
        :ui="{ label: 'truncate' }"
        data-testid="link-open"
      />
      <UButton type="button" color="neutral" variant="ghost" size="xs" square icon="i-hugeicons-pencil-edit-02" title="Edit link" aria-label="Edit link" data-testid="link-edit" @mousedown.prevent @click="emit('edit')" />
      <UButton type="button" color="neutral" variant="ghost" size="xs" square icon="i-hugeicons-unlink-01" title="Remove link" aria-label="Remove link" data-testid="link-remove" @mousedown.prevent @click="emit('remove')" />
    </div>
  </div>
</template>
