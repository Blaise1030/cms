<script setup lang="ts">
import UCard from "@nuxt/ui/components/Card.vue";
import UCarousel from "@nuxt/ui/components/Carousel.vue";
import type { AppShotData } from "../../lib/appShotTypes";
import { paintingUrl, type PaintingId } from "../../lib/paintings";
import AppShot from "./AppShot.vue";

const slides = [
  {
    title: "Collections you can browse",
    caption: "Every Markdown collection in the repo, as a list with status.",
    kind: "content",
    painting: "danube",
    alt: "Orso editing a blog post: the body on the left, its fields (slug, title, date, tags) in a form on the right.",
  },
  {
    title: "Desktop and mobile preview",
    caption: "Share a draft link, leave comments, approve before Publish.",
    kind: "preview",
    painting: "bridge",
    alt: "Orso previewing a draft of the site, with the agent panel open beside it.",
  },
  {
    title: "Publish with a checklist",
    caption: "Build, links and review run first. One button puts it live.",
    kind: "publish",
    painting: "toledo",
    alt: "Orso's changes menu listing the live site and open drafts.",
  },
  {
    title: "Ask in plain English",
    caption: "Whole pages, a pass on the FAQ, or a new section — as a draft.",
    kind: "agent",
    painting: "alps",
    alt: "Orso's agent panel beside the site preview.",
  },
  {
    title: "Checkpoints you can restore",
    caption: "Every save is history. Restoring makes a new draft, never a rewrite.",
    kind: "history",
    painting: "rocks",
    alt: "An Orso checkpoint opened from the history timeline.",
  },
] as const satisfies readonly { kind: string; painting: PaintingId }[];
type SlideKind = (typeof slides)[number]["kind"];

defineProps<{ shots: Record<SlideKind, AppShotData> }>();
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
            :shot="shots[item.kind]"
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
