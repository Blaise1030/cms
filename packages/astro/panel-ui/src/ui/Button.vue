<script setup lang="ts">
import { cn } from "./cn";
import { buttonVariants, type ButtonVariants } from "./button";

const props = withDefaults(
  defineProps<{
    variant?: ButtonVariants["variant"];
    size?: ButtonVariants["size"];
    class?: string;
    type?: "button" | "submit" | "reset";
  }>(),
  { type: "button" },
);
</script>

<template>
  <button
    :type="props.type"
    :class="cn(buttonVariants({ variant: props.variant, size: props.size }), props.class)"
  >
    <slot />
  </button>
</template>
