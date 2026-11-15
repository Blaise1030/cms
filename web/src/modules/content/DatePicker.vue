<script setup lang="ts">
// A date field for frontmatter: a button showing the date ("Oct 28, 2026") that opens Nuxt UI's
// calendar. The value stays an ISO date string (`2026-10-28`); a datetime keeps only its date part.
import { computed, ref } from "vue";
import { CalendarDate, DateFormatter, getLocalTimeZone, parseDate } from "@internationalized/date";
import UButton from "@nuxt/ui/components/Button.vue";
import UCalendar from "@nuxt/ui/components/Calendar.vue";
import UPopover from "@nuxt/ui/components/Popover.vue";

defineOptions({ inheritAttrs: false });

const props = defineProps<{
  modelValue?: string;
  placeholder?: string;
  /** Offers Clear, which emits `undefined`. */
  clearable?: boolean;
  id?: string;
}>();
const emit = defineEmits<{ "update:modelValue": [value: string | undefined] }>();

const open = ref(false);
const formatter = new DateFormatter("en", { dateStyle: "medium" });

const date = computed(() => {
  if (!props.modelValue) return undefined;
  try {
    return parseDate(props.modelValue.slice(0, 10));
  } catch {
    return undefined;
  }
});
const label = computed(() =>
  date.value ? formatter.format(date.value.toDate(getLocalTimeZone())) : props.modelValue,
);

function choose(value: unknown) {
  if (!(value instanceof CalendarDate)) return;
  emit("update:modelValue", value.toString());
  open.value = false;
}
function clear() {
  emit("update:modelValue", undefined);
  open.value = false;
}
</script>

<template>
  <UPopover v-model:open="open" :content="{ align: 'start' }">
    <UButton
      v-bind="$attrs"
      :id="id"
      color="neutral"
      variant="outline"
      block
      icon="i-hugeicons-calendar-03"
      class="justify-start font-normal"
      :class="!modelValue && 'text-muted'"
      :label="label || placeholder || 'Pick a date'"
    />
    <template #content>
      <div class="p-2" data-testid="date-picker-calendar">
        <UCalendar :model-value="date" :default-placeholder="date" year-controls @update:model-value="choose" />
      </div>
      <div v-if="clearable && modelValue" class="border-default border-t p-2">
        <UButton
          color="neutral"
          variant="ghost"
          size="sm"
          block
          label="Clear"
          data-testid="date-picker-clear"
          @click="clear"
        />
      </div>
    </template>
  </UPopover>
</template>
