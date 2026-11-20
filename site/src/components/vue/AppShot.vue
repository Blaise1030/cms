<script setup lang="ts">
import type { AppShotData } from "../../lib/appShotTypes";

/**
 * A real Orso app capture in the site's current colour scheme. The light/dark pair switches on
 * prefers-color-scheme, the same signal the design tokens follow.
 */
const props = withDefaults(
  defineProps<{
    shot: AppShotData;
    alt: string;
    /** `sizes` for the srcset; defaults to the full viewport width. */
    sizes?: string;
    eager?: boolean;
    /** Play the recorded clip (when the shot has one); reduced motion keeps the still. */
    play?: boolean;
  }>(),
  { sizes: "100vw", eager: false, play: false },
);
</script>

<template>
  <div
    class="relative overflow-hidden rounded-md border border-line bg-surface"
    :style="{ aspectRatio: `${props.shot.width} / ${props.shot.height}` }"
  >
    <picture>
      <source media="(prefers-color-scheme: dark)" :srcset="props.shot.dark.srcSet" :sizes="props.sizes" />
      <img
        :src="props.shot.light.src"
        :srcset="props.shot.light.srcSet"
        :sizes="props.sizes"
        :width="props.shot.width"
        :height="props.shot.height"
        :alt="props.alt"
        :loading="props.eager ? 'eager' : 'lazy'"
        decoding="async"
        class="block size-full object-cover object-top-left"
      />
    </picture>
    <template v-if="props.play && props.shot.video">
      <video
        v-for="scheme in (['light', 'dark'] as const)"
        :key="scheme"
        :src="props.shot.video[scheme]"
        class="absolute inset-0 size-full object-cover object-top-left motion-reduce:hidden!"
        :class="scheme === 'light' ? 'scheme-light' : 'scheme-dark'"
        autoplay
        muted
        loop
        playsinline
        preload="metadata"
        aria-hidden="true"
      />
    </template>
  </div>
</template>
