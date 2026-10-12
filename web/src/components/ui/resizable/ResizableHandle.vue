<script setup lang="ts">
import type { SplitterResizeHandleEmits, SplitterResizeHandleProps } from "reka-ui"
import type { HTMLAttributes } from "vue"
import { reactiveOmit } from "@vueuse/core"
import { SplitterResizeHandle, useForwardPropsEmits } from "reka-ui"
import { cn } from "@/lib/utils"

const props = defineProps<SplitterResizeHandleProps & { class?: HTMLAttributes["class"], withHandle?: boolean }>()
const emits = defineEmits<SplitterResizeHandleEmits>()

const delegatedProps = reactiveOmit(props, "class", "withHandle")
const forwarded = useForwardPropsEmits(delegatedProps, emits)
</script>

<template>
  <SplitterResizeHandle
    data-slot="resizable-handle"
    v-bind="forwarded"
    :class="cn('group bg-border focus-visible:ring-ring relative flex w-px items-center justify-center after:absolute after:inset-y-0 after:left-1/2 after:w-2 after:-translate-x-1/2 focus-visible:ring-1 focus-visible:ring-offset-1 focus-visible:outline-hidden data-[orientation=vertical]:h-px data-[orientation=vertical]:w-full data-[orientation=vertical]:after:left-0 data-[orientation=vertical]:after:h-2 data-[orientation=vertical]:after:w-full data-[orientation=vertical]:after:-translate-y-1/2 data-[orientation=vertical]:after:translate-x-0', props.class)"
  >
    <template v-if="props.withHandle">
      <slot>
        <div class="bg-muted-foreground/50 group-hover:bg-foreground/70 group-data-[resize-handle-state=drag]:bg-foreground z-10 h-8 w-1 shrink-0 rounded-full group-data-[orientation=vertical]:h-1 group-data-[orientation=vertical]:w-8" />
      </slot>
    </template>
  </SplitterResizeHandle>
</template>
