<script setup lang="ts">
import { computed, inject, onBeforeUnmount, ref, watch } from "vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UModal from "@nuxt/ui/components/Modal.vue";
import { Markdown, type StreamMarkdownParserOptions } from "vue-stream-markdown";
import "vue-stream-markdown/index.css";
import { openWebLink } from "@/modules/chat/links";
import { ASSET_IMAGES } from "./assetImages";
import { splitImageRuns, stripHtmlComments } from "./markdownText";

/** Markdown a teammate wrote on GitHub (untrusted): raw HTML stays escaped text, links open outside. */
const props = defineProps<{ content: string }>();
/** Review images on the repo's assets branch load through the daemon (private repos too). */
const rewrite = inject(ASSET_IMAGES, (markdown: string) => markdown);
// HTML comments are hidden, as on GitHub (bots leave markers in them). Neighbouring images
// (attachments) show as a grid of thumbnails; text, and an image under its own note, stay in place.
const blocks = computed(() => splitImageRuns(rewrite(stripHtmlComments(props.content))));
// Replacing the parser's default `html` plugin leaves raw HTML as escaped text.
const parserOptions: StreamMarkdownParserOptions = { plugins: [{ name: "html" }] };
/** Only web and same-origin images become links (`javascript:` and the like never do). */
const safe = (url: string) => /^(https?:\/\/|\/)/i.test(url);

/** Spotlight: one image of a grid shown large, with ←/→ through that grid. */
const spotlight = ref<{ images: { alt: string; url: string }[]; index: number } | null>(null);
const current = computed(() => (spotlight.value ? spotlight.value.images[spotlight.value.index] : null));
function openSpotlight(images: { alt: string; url: string }[], index: number) {
  spotlight.value = { images: images.filter((i) => safe(i.url)), index };
}
function step(by: number) {
  const s = spotlight.value;
  if (!s || s.images.length < 2) return;
  s.index = (s.index + by + s.images.length) % s.images.length;
}
function onKey(event: KeyboardEvent) {
  if (event.key === "ArrowRight") step(1);
  else if (event.key === "ArrowLeft") step(-1);
}
// ←/→ work wherever focus is while the spotlight is open.
watch(
  () => !!spotlight.value,
  (open) => (open ? window.addEventListener("keydown", onKey) : window.removeEventListener("keydown", onKey)),
);
onBeforeUnmount(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <div class="text-default flex min-w-0 flex-col gap-2 text-sm" data-testid="review-markdown" @click.capture="openWebLink">
    <template v-for="(block, i) in blocks" :key="i">
      <!-- Static: the text is complete, so the last block never waits on more input. -->
      <Markdown v-if="block.kind === 'text'" mode="static" :content="block.text" :parser-options="parserOptions" />
      <div
        v-else
        class="grid grid-cols-[repeat(auto-fill,minmax(10rem,1fr))] gap-2"
        data-testid="review-image-grid"
      >
        <template v-for="(image, j) in block.images.filter((i) => safe(i.url))" :key="j">
          <button
            type="button"
            class="border-default bg-elevated/40 hover:ring-primary/60 block aspect-[4/3] cursor-zoom-in overflow-hidden rounded-md border hover:ring-2"
            :aria-label="`View ${image.alt || 'image'} larger`"
            data-testid="review-image-thumb"
            @click="openSpotlight(block.images, j)"
          >
            <img :src="image.url" :alt="image.alt" loading="lazy" class="size-full object-cover" />
          </button>
        </template>
      </div>
    </template>
    <UModal
      :open="!!spotlight"
      fullscreen
      :title="current?.alt || 'Image'"
      :description="spotlight && spotlight.images.length > 1 ? `${spotlight.index + 1} of ${spotlight.images.length}` : undefined"
      :ui="{ body: 'flex min-h-0 flex-1 items-center justify-center gap-2 p-2 sm:p-4' }"
      @update:open="(open: boolean) => !open && (spotlight = null)"
    >
      <template #body>
        <div class="flex size-full min-h-0 items-center gap-2" data-testid="review-spotlight">
          <UButton
            v-if="spotlight && spotlight.images.length > 1"
            color="neutral"
            variant="ghost"
            icon="i-hugeicons-arrow-left-01"
            aria-label="Previous image"
            data-testid="review-spotlight-prev"
            @click="step(-1)"
          />
          <img
            v-if="current"
            :src="current.url"
            :alt="current.alt"
            class="mx-auto max-h-full min-h-0 max-w-full min-w-0 flex-1 object-contain"
            data-testid="review-spotlight-image"
          />
          <UButton
            v-if="spotlight && spotlight.images.length > 1"
            color="neutral"
            variant="ghost"
            icon="i-hugeicons-arrow-right-01"
            aria-label="Next image"
            data-testid="review-spotlight-next"
            @click="step(1)"
          />
        </div>
      </template>
      <template #footer>
        <a
          v-if="current"
          :href="current.url"
          target="_blank"
          rel="noopener noreferrer"
          class="text-muted hover:text-highlighted text-sm underline-offset-2 hover:underline"
        >Open original</a>
      </template>
    </UModal>
  </div>
</template>
