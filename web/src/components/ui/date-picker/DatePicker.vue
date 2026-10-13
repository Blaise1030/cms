<script setup lang="ts">
import type { DateValue } from "reka-ui"
import type { HTMLAttributes } from "vue"
import { DateFormatter, getLocalTimeZone, parseDate } from "@internationalized/date"
import { CalendarIcon } from "@lucide/vue"
import { computed, ref } from "vue"
import { Button } from "@/components/ui/button"
import { Calendar } from "@/components/ui/calendar"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { cn } from "@/lib/utils"

defineOptions({ inheritAttrs: false })

const props = defineProps<{
  /** An ISO date (`2026-01-02`); a datetime keeps only its date part. */
  modelValue?: string
  placeholder?: string
  /** Offers Clear, which emits `undefined`. */
  clearable?: boolean
  id?: string
  class?: HTMLAttributes["class"]
}>()
const emit = defineEmits<{ "update:modelValue": [value: string | undefined] }>()

const open = ref(false)
const formatter = new DateFormatter("en", { dateStyle: "medium" })

const date = computed(() => {
  if (!props.modelValue) return undefined
  try { return parseDate(props.modelValue.slice(0, 10)) } catch { return undefined }
})
const label = computed(() => date.value ? formatter.format(date.value.toDate(getLocalTimeZone())) : props.modelValue)

function choose(value: DateValue | undefined) {
  if (!value) return
  emit("update:modelValue", value.toString())
  open.value = false
}
function clear() {
  emit("update:modelValue", undefined)
  open.value = false
}
</script>

<template>
  <Popover v-model:open="open">
    <PopoverTrigger as-child>
      <Button
        v-bind="$attrs"
        :id="id"
        type="button"
        variant="outline"
        data-slot="date-picker"
        :class="cn('w-full justify-start font-normal', !modelValue && 'text-muted-foreground', props.class)"
      >
        <CalendarIcon />
        {{ label || placeholder || "Pick a date" }}
      </Button>
    </PopoverTrigger>
    <PopoverContent class="w-auto p-0" align="start">
      <Calendar
        :model-value="date"
        :default-placeholder="date"
        layout="month-and-year"
        initial-focus
        @update:model-value="choose($event as DateValue | undefined)"
      />
      <div v-if="clearable && modelValue" class="border-t p-2">
        <Button type="button" variant="ghost" size="sm" class="w-full" data-slot="date-picker-clear" @click="clear">
          Clear
        </Button>
      </div>
    </PopoverContent>
  </Popover>
</template>
