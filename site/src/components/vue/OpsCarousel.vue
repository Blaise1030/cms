<script setup lang="ts">
import UCard from "@nuxt/ui/components/Card.vue";
import UCarousel from "@nuxt/ui/components/Carousel.vue";
import type { AppShotData } from "../../lib/appShotTypes";
import { paintingUrl, type PaintingId } from "../../lib/paintings";
import AppShot from "./AppShot.vue";

const paintings = {
  content: "danube",
  preview: "bridge",
  publish: "toledo",
  agent: "alps",
  history: "rocks",
} as const satisfies Record<string, PaintingId>;
type SlideKind = keyof typeof paintings;
type SlideCopy = { title: string; caption: string; alt: string };

/** `copy` is `home.ops.slides` from src/i18n/messages, one entry per slide. */
const props = defineProps<{ shots: Record<SlideKind, AppShotData>; copy: Record<SlideKind, SlideCopy> }>();
const slides = (Object.keys(paintings) as SlideKind[]).map((kind) => ({ kind, painting: paintings[kind], ...props.copy[kind] }));
</script>

<template>
  <UCarousel
    v-slot="{ item }"
    :items="[...slides]"
    arrows
    align="start"
    prev-icon="i-hugeicons-arrow-left-01"
    next-icon="i-hugeicons-arrow-right-01"
    :prev="{ size: 'lg' }"
    :next="{ size: 'lg' }"
    class="flex w-full flex-col"
    :ui="{
      viewport: 'border-t border-line',
      container: 'ms-0 items-stretch',
      item: 'basis-4/5 border-r border-line ps-0 last:border-r-0 md:basis-1/2 lg:basis-2/5',
      controls: 'order-first',
      arrows: 'page mb-6 flex items-center justify-start gap-2 md:justify-end',
      prev: 'static translate-none',
      next: 'static translate-none',
    }"
  >
    <UCard
      variant="ghost"
      class="flex h-full flex-col rounded-none bg-transparent p-2"
      :ui="{ header: 'relative shrink-0 overflow-hidden rounded-md border border-line p-0 sm:p-0', body: 'shrink-0 px-4 pt-5 pb-4 sm:px-4 sm:pt-5 sm:pb-4' }"
    >
      <template #header>
        <img
          :src="paintingUrl(item.painting)"
          alt=""
          aria-hidden="true"
          loading="lazy"
          decoding="async"
          class="absolute inset-0 size-full object-cover"
        />
        <div class="absolute inset-0 bg-night/20" aria-hidden="true" />
        <div class="relative p-2 md:p-3">
          <AppShot
            :shot="props.shots[item.kind]"
            :alt="item.alt"
            sizes="(min-width: 64rem) 32rem, (min-width: 48rem) 50vw, 80vw"
            class="aspect-3/2 w-full"
          />
        </div>
      </template>
      <h3 class="text-body font-medium tracking-normal text-ink">{{ item.title }}</h3>
      <p class="mt-1 text-small text-muted">{{ item.caption }}</p>
    </UCard>
  </UCarousel>
</template>
