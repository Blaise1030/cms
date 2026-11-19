<script setup lang="ts">
import { computed, inject } from "vue";
import { Markdown, type StreamMarkdownParserOptions } from "vue-stream-markdown";
import "vue-stream-markdown/index.css";
import { openWebLink } from "@/modules/chat/links";
import { ASSET_IMAGES } from "./assetImages";
import { stripHtmlComments } from "./markdownText";

/** Markdown a teammate wrote on GitHub (untrusted): raw HTML stays escaped text, links open outside. */
const props = defineProps<{ content: string }>();
/** Review images on the repo's assets branch load through the daemon (private repos too). */
const rewrite = inject(ASSET_IMAGES, (markdown: string) => markdown);
// HTML comments are hidden, as on GitHub (bots leave markers in them).
const shown = computed(() => rewrite(stripHtmlComments(props.content)));
// Replacing the parser's default `html` plugin leaves raw HTML as escaped text.
const parserOptions: StreamMarkdownParserOptions = { plugins: [{ name: "html" }] };
</script>

<template>
  <div class="text-default min-w-0 text-sm" data-testid="review-markdown" @click.capture="openWebLink">
    <!-- Static: the text is complete, so the last block (often an image) never waits on more input. -->
    <Markdown mode="static" :content="shown" :parser-options="parserOptions" />
  </div>
</template>
