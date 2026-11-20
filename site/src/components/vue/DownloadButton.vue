<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import type { DropdownMenuItem } from "@nuxt/ui";
import UButton from "@nuxt/ui/components/Button.vue";
import UDropdownMenu from "@nuxt/ui/components/DropdownMenu.vue";
import UFieldGroup from "@nuxt/ui/components/FieldGroup.vue";
import UModal from "@nuxt/ui/components/Modal.vue";
import { site } from "../../site";

const { version, repo, coffeeUrl, downloads } = site.release;
type Download = (typeof downloads)[number];

const assetUrl = (target: string) => `${repo}/releases/download/${version}/orso-${version}-${target}.tar.gz`;

/** Picks the build for the visitor's OS after hydration; macOS Apple Silicon is the server-rendered default. */
function detectDownload(): Download {
  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes("linux") && !ua.includes("android")) return downloads.find((d) => d.os === "linux") ?? downloads[0];
  return downloads[0];
}

const primary = ref<Download>(downloads[0]);
onMounted(() => (primary.value = detectDownload()));

/** The link still downloads the file; the thank-you popup opens alongside it. */
const downloaded = ref<Download | null>(null);
const open = computed({
  get: () => downloaded.value !== null,
  set: (value) => { if (!value) downloaded.value = null; },
});

const items = computed<DropdownMenuItem[][]>(() => [
  downloads.map((d) => ({
    label: d.label,
    detail: `${d.detail} .tar.gz`,
    to: assetUrl(d.target),
    target: "_self",
    rel: "noopener noreferrer",
    slot: "download" as const,
    onSelect: () => (downloaded.value = d),
  })),
  [{ label: "All releases on GitHub", to: `${repo}/releases/latest`, rel: "noopener noreferrer", class: "text-muted" }],
]);
</script>

<template>
  <UFieldGroup>
    <UButton
      size="lg"
      icon="i-hugeicons-download-04"
      :label="`Download for ${primary.label}`"
      :to="assetUrl(primary.target)"
      target="_self"
      rel="noopener noreferrer"
      @click="downloaded = primary"
    />
    <UDropdownMenu :items="items" :content="{ align: 'end' }" :ui="{ content: 'w-64' }">
      <UButton
        size="lg"
        icon="i-hugeicons-arrow-down-01"
        aria-label="Choose another platform"
        class="border-l border-on-accent/20"
      />
      <template #download-label="{ item }">
        <span class="flex w-full justify-between gap-3">
          <span class="text-ink">{{ item.label }}</span>
          <span class="text-muted">{{ item.detail }}</span>
        </span>
      </template>
    </UDropdownMenu>
  </UFieldGroup>

  <UModal
    v-model:open="open"
    title="Thanks for downloading Orso"
    description="Your download should start in a moment."
    :ui="{ overlay: 'z-50 bg-night/30 backdrop-blur-md', content: 'z-50 max-w-md', title: 'text-body font-medium text-ink', description: 'text-small text-muted' }"
  >
    <template #body>
      <div class="space-y-3 text-small text-muted">
        <template v-if="downloaded?.os === 'mac'">
          <p>
            Orso isn't notarized by Apple yet, so macOS may block it the first time. After unpacking, run this once to
            allow it:
          </p>
          <code class="block overflow-x-auto rounded-md border border-line bg-surface px-3 py-2 whitespace-nowrap text-ink">
            xattr -d com.apple.quarantine ./orso
          </code>
          <p>Notarizing costs $99 a year. If Orso is useful to you, a coffee helps cover it.</p>
        </template>
        <p v-else>Orso is free and built in my spare time. If it's useful to you, a coffee helps keep it going.</p>
      </div>
    </template>
    <template #footer="{ close }">
      <div class="flex flex-wrap gap-3">
        <UButton size="lg" icon="i-hugeicons-coffee-02" label="Buy me a coffee" :to="coffeeUrl" target="_blank" rel="noopener noreferrer" />
        <UButton size="lg" color="neutral" variant="outline" label="Maybe later" @click="close" />
      </div>
    </template>
  </UModal>
</template>
