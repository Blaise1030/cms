<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import type { DropdownMenuItem } from "@nuxt/ui";
import UButton from "@nuxt/ui/components/Button.vue";
import UDropdownMenu from "@nuxt/ui/components/DropdownMenu.vue";
import UFieldGroup from "@nuxt/ui/components/FieldGroup.vue";
import UModal from "@nuxt/ui/components/Modal.vue";
import { site } from "../../site";

const { repo, coffeeUrl, downloads } = site.release;

/** The button's copy, from `download` in src/i18n/messages (`{platform}` is filled in here). */
export type DownloadLabels = {
  button: string;
  otherPlatform: string;
  allReleases: string;
  thanksTitle: string;
  thanksDescription: string;
  macSteps: string;
  macCost: string;
  otherNote: string;
  coffee: string;
  later: string;
};
const props = defineProps<{ labels: DownloadLabels }>();
type Download = (typeof downloads)[number];

/** `releases/latest/download` redirects to the newest release's asset, so no version is in the URL. */
const assetUrl = (d: { target: string; ext: string }) => `${repo}/releases/latest/download/orso-${d.target}.${d.ext}`;

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
    detail: `${d.detail} .${d.ext}`,
    to: assetUrl(d),
    target: "_self",
    rel: "noopener noreferrer",
    slot: "download" as const,
    onSelect: () => (downloaded.value = d),
  })),
  [{ label: props.labels.allReleases, to: `${repo}/releases/latest`, rel: "noopener noreferrer", class: "text-muted" }],
]);
</script>

<template>
  <UFieldGroup>
    <UButton
      size="lg"
      icon="i-hugeicons-download-04"
      :label="props.labels.button.replace('{platform}', primary.label)"
      :to="assetUrl(primary)"
      target="_self"
      rel="noopener noreferrer"
      @click="downloaded = primary"
    />
    <UDropdownMenu :items="items" :content="{ align: 'end' }" :ui="{ content: 'w-64' }">
      <UButton
        size="lg"
        icon="i-hugeicons-arrow-down-01"
        :aria-label="props.labels.otherPlatform"
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
    :title="props.labels.thanksTitle"
    :description="props.labels.thanksDescription"
    :ui="{ overlay: 'z-50 bg-night/30 backdrop-blur-md', content: 'z-50 max-w-md', title: 'text-body font-medium text-ink', description: 'text-small text-muted' }"
  >
    <template #body>
      <div class="space-y-3 text-small text-muted">
        <template v-if="downloaded?.os === 'mac'">
          <p>{{ props.labels.macSteps }}</p>
          <code class="block overflow-x-auto rounded-md border border-line bg-surface px-3 py-2 whitespace-nowrap text-ink">
            xattr -dr com.apple.quarantine /Applications/orso.app
          </code>
          <p>{{ props.labels.macCost }}</p>
        </template>
        <p v-else>{{ props.labels.otherNote }}</p>
      </div>
    </template>
    <template #footer="{ close }">
      <div class="flex flex-wrap gap-3">
        <!-- Buy Me a Coffee's own button look; their embed script relies on document.write, so it can't render in a modal. -->
        <a
          :href="coffeeUrl"
          target="_blank"
          rel="noopener noreferrer"
          class="inline-flex h-11 items-center gap-2 rounded-lg border border-black bg-[#FFDD00] px-4 font-(family-name:--font-cookie) text-[1.6rem] leading-none text-black transition hover:opacity-85 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-black"
        >
          <!-- Cup drawn inline (white coffee, black outline) so it never depends on a third-party image. -->
          <svg viewBox="0 0 24 32" width="18" height="24" aria-hidden="true" fill="none" stroke="#000" stroke-width="1.5" stroke-linejoin="round">
            <path d="M2 7h20l-1 3H3z" fill="#fff" />
            <path d="M4 4h16l1.5 3h-19z" fill="#fff" />
            <path d="M3.5 10h17L18 30H6z" fill="#fff" />
            <path d="M4.6 15h14.8l-.9 7H5.5z" fill="#FFDD00" />
          </svg>
          {{ props.labels.coffee }}
        </a>
        <UButton size="lg" color="neutral" variant="outline" :label="props.labels.later" @click="close" />
      </div>
    </template>
  </UModal>
</template>
