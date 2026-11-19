<script setup lang="ts">
import { computed, inject } from "vue";
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
        <template v-for="(image, j) in block.images" :key="j">
          <a
            v-if="safe(image.url)"
            :href="image.url"
            target="_blank"
            rel="noopener noreferrer"
            class="border-default bg-elevated/40 block aspect-[4/3] overflow-hidden rounded-md border"
            :title="image.alt || 'Open image'"
          >
            <img :src="image.url" :alt="image.alt" loading="lazy" class="size-full object-cover" />
          </a>
        </template>
      </div>
    </template>
  </div>
</template>
