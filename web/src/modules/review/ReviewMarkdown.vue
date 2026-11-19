<script setup lang="ts">
import { Markdown, type StreamMarkdownParserOptions } from "vue-stream-markdown";
import "vue-stream-markdown/index.css";
import { openWebLink } from "@/modules/chat/links";

/** Markdown a teammate wrote on GitHub (untrusted): raw HTML stays escaped text, links open outside. */
defineProps<{ content: string }>();
// Replacing the parser's default `html` plugin leaves raw HTML as escaped text.
const parserOptions: StreamMarkdownParserOptions = { plugins: [{ name: "html" }] };
</script>

<template>
  <div class="text-default min-w-0 text-sm" data-testid="review-markdown" @click.capture="openWebLink">
    <Markdown :content="content" :parser-options="parserOptions" />
  </div>
</template>
