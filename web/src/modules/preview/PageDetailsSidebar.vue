<script setup lang="ts">
import { computed, ref } from "vue";
import type { TabsItem } from "@nuxt/ui";
import UBadge from "@nuxt/ui/components/Badge.vue";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UCard from "@nuxt/ui/components/Card.vue";
import UEmpty from "@nuxt/ui/components/Empty.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import UTabs from "@nuxt/ui/components/Tabs.vue";
import { useDaemon } from "@/core/daemon";
import type { Finding } from "./checks/findings";
import { DESCRIPTION_MAX, TITLE_MAX, type PageMeta } from "./checks/meta";
import { previewMediaSrc } from "./checks/previewAsset";
import { formatBytes, type ResourceSummary } from "./checks/resourceSummary";

const props = defineProps<{
  meta: PageMeta | null;
  findings: Finding[];
  resources: ResourceSummary | null;
  path: string;
  /** Absolute URL of the framed page (used to load favicon / OG from the live preview). */
  pageUrl: string;
}>();

const daemon = useDaemon();

const emit = defineEmits<{
  close: [];
  refresh: [];
  fix: [text: string];
}>();

type Tab = "meta" | "issues";
const tab = ref<Tab>("meta");

function display(value: string) {
  return value.trim() ? value : null;
}

function h1Label(m: PageMeta) {
  if (m.h1Count === 0) return null;
  if (m.h1Count === 1) return m.h1;
  return `${m.h1} (${m.h1Count} H1s)`;
}

function fixPrompt(finding: Finding) {
  const lines = [`The preview checks found an issue on ${finding.page || props.path}: ${finding.message}`];
  if (finding.detail) lines.push(finding.detail);
  lines.push(
    "",
    "Fix it in the site and keep the existing copy and design.",
    "Rely on HMR; do not run a build or publish.",
  );
  return lines.join("\n");
}

type MetaRow = {
  key: string;
  label: string;
  value: string | null;
  hint?: string;
  mono?: boolean;
};

type MetaGroup = { id: string; title: string; rows: MetaRow[] };

const metaGroups = computed((): MetaGroup[] => {
  const m = props.meta;
  if (!m) return [];
  return [
    {
      id: "page",
      title: "Page",
      rows: [
        {
          key: "title",
          label: "Title",
          value: display(m.title),
          hint: `${m.titleLength}/${TITLE_MAX}`,
        },
        {
          key: "description",
          label: "Description",
          value: display(m.description),
          hint: `${m.descriptionLength}/${DESCRIPTION_MAX}`,
        },
        { key: "h1", label: "Heading (H1)", value: h1Label(m) },
        { key: "canonical", label: "Canonical URL", value: display(m.canonical), mono: true },
        { key: "robots", label: "Robots", value: display(m.robots) },
        { key: "lang", label: "Language", value: display(m.lang) },
      ],
    },
    {
      id: "social",
      title: "Social preview",
      rows: [
        { key: "ogTitle", label: "OG title", value: display(m.ogTitle) },
        { key: "ogDescription", label: "OG description", value: display(m.ogDescription) },
      ],
    },
  ];
});

const faviconUrl = computed(() => display(props.meta?.favicon ?? ""));
const ogImageUrl = computed(() => display(props.meta?.ogImage ?? ""));

/**
 * Img src: map the site path onto the framed preview, then (when possible) serve it
 * same-origin via the UI `/preview` proxy — raw `127.0.0.1:<astro>` URLs break in the shell.
 * The visible label stays the path-only `faviconUrl` / `ogImageUrl`.
 */
function mediaSrc(href: string | null): string | null {
  if (!href || !props.pageUrl) return href;
  return (
    previewMediaSrc(href, props.pageUrl, {
      uiOrigin: window.location.origin,
      token: daemon.config.value.token,
    }) || null
  );
}

const faviconSrc = computed(() => mediaSrc(faviconUrl.value));
const ogImageSrc = computed(() => mediaSrc(ogImageUrl.value));

const jsonLdBlocks = computed(() => props.meta?.jsonLdBlocks ?? []);
const jsonLdTypesLabel = computed(() =>
  props.meta?.jsonLdTypes.length ? props.meta.jsonLdTypes.join(", ") : null,
);

const issueCount = computed(() => props.findings.length);

const CARD_UI = { header: 'px-3 py-2 sm:px-3', title: 'text-xs tracking-tight', body: 'divide-default divide-y p-0 sm:p-0' };

const tabs = computed<TabsItem[]>(() => [
  { label: "Meta", icon: "i-hugeicons-tag-01", value: "meta", slot: "meta" },
  { label: "Issues", icon: "i-hugeicons-alert-circle", value: "issues", slot: "issues" },
]);
</script>

<template>
  <aside
    class="bg-background flex h-full min-h-0 w-full min-w-0 flex-col"
    data-testid="page-details-sidebar"
  >
    <header class="bg-background shrink-0 px-3 pt-2">
      <div class="flex items-center justify-between gap-2 pb-2">
        <div class="text-highlighted text-sm font-medium tracking-tight">Page details</div>
        <div class="flex items-center gap-1">
          <UButton
            color="neutral"
            variant="outline"
            size="xs"
            icon="i-hugeicons-refresh"
            label="Refresh"
            data-testid="page-details-refresh"
            @click="emit('refresh')"
          />
          <UButton
            color="neutral"
            variant="ghost"
            size="xs"
            icon="i-hugeicons-arrow-right-double"
            aria-label="Close page details"
            data-testid="page-details-close"
            @click="emit('close')"
          />
        </div>
      </div>
      <div
        class="text-muted flex flex-wrap items-center gap-x-3 gap-y-1 pb-2 text-xs"
        data-testid="page-details-summary"
      >
        <template v-if="resources">
          <span class="inline-flex items-center gap-1" title="Scripts">
            <UIcon name="i-hugeicons-code" class="size-3 shrink-0" aria-hidden="true" />
            <span class="tabular-nums">{{ resources.scripts }}</span>
          </span>
          <span class="inline-flex items-center gap-1" title="Styles">
            <UIcon name="i-hugeicons-paint-board" class="size-3 shrink-0" aria-hidden="true" />
            <span class="tabular-nums">{{ resources.styles }}</span>
          </span>
          <span class="inline-flex items-center gap-1" title="Images">
            <UIcon name="i-hugeicons-image-01" class="size-3 shrink-0" aria-hidden="true" />
            <span class="tabular-nums">{{ resources.images }}</span>
          </span>
          <span class="inline-flex items-center gap-1" title="HTML size">
            <UIcon name="i-hugeicons-file-script" class="size-3 shrink-0" aria-hidden="true" />
            <span class="tabular-nums">{{ formatBytes(resources.htmlBytes) }}</span>
          </span>
        </template>
        <span v-else>Open a page to inspect SEO fields.</span>
      </div>
    </header>

    <UTabs
      :model-value="tab"
      :items="tabs"
      size="xs"
      color="neutral"
      variant="pill"
      class="min-h-0 flex-1 gap-0"
      :ui="{ root: 'items-stretch', list: 'mx-3', trigger: 'flex-1 justify-center data-[state=active]:text-highlighted', content: 'min-h-0 flex-1 overflow-auto' }"
      @update:model-value="tab = $event as Tab"
    >
      <template #default="{ item }">
        <span :data-testid="`page-details-tab-${item.value}`">
          {{ item.label }}<span
            v-if="item.value === 'issues' && issueCount > 0"
            class="text-muted ml-1 tabular-nums"
          >{{ issueCount }}</span>
        </span>
      </template>

      <template #meta>
        <div class="h-full" data-testid="page-details-meta">
          <div v-if="metaGroups.length" class="flex flex-col gap-5 px-3 pt-3 pb-3">
            <UCard title="Branding" :ui="CARD_UI">
              <div class="grid gap-2 px-3 py-3">
                <div class="text-muted text-xs font-medium">Favicon</div>
                <div v-if="faviconUrl" class="flex items-center gap-3">
                  <img
                    :src="faviconSrc ?? faviconUrl"
                    alt=""
                    class="border-default bg-default size-8 rounded-md border object-contain p-1"
                    data-testid="page-details-favicon"
                  />
                  <span class="text-muted font-mono text-xs break-all" :title="faviconUrl">
                    {{ faviconUrl }}
                  </span>
                </div>
                <p v-else class="text-dimmed text-sm italic">Missing</p>
              </div>
              <div class="grid gap-2 px-3 py-3">
                <div class="text-muted text-xs font-medium">OG image</div>
                <div v-if="ogImageUrl" class="grid gap-2">
                  <img
                    :src="ogImageSrc ?? ogImageUrl"
                    alt=""
                    class="border-default bg-muted max-h-40 w-full rounded-md border object-cover"
                    data-testid="page-details-og-image"
                  />
                  <span class="text-muted font-mono text-xs break-all" :title="ogImageUrl">
                    {{ ogImageUrl }}
                  </span>
                </div>
                <p v-else class="text-dimmed text-sm italic">Missing</p>
              </div>
            </UCard>

            <UCard v-for="group in metaGroups" :key="group.id" :title="group.title" :ui="CARD_UI">
              <dl class="divide-default m-0 divide-y">
                <div
                  v-for="row in group.rows"
                  :key="row.key"
                  class="grid gap-1.5 px-3 py-3"
                >
                  <div class="flex items-baseline justify-between gap-3">
                    <dt class="text-muted text-xs font-medium">
                      {{ row.label }}
                    </dt>
                    <UBadge
                      v-if="row.hint"
                      color="neutral"
                      variant="soft"
                      size="sm"
                      class="tabular-nums"
                      :label="row.hint"
                    />
                  </div>
                  <dd
                    class="m-0 text-sm leading-relaxed"
                    :class="[
                      row.value ? 'text-default' : 'text-dimmed italic',
                      row.mono ? 'font-mono text-xs break-all' : 'break-words',
                    ]"
                    :title="row.value ?? undefined"
                  >
                    {{ row.value ?? "Missing" }}
                  </dd>
                </div>
              </dl>
            </UCard>

            <UCard data-testid="page-details-jsonld" :ui="CARD_UI">
              <template #header>
                <div class="flex items-baseline justify-between gap-3">
                  <h3 class="text-highlighted text-xs font-semibold tracking-tight">Structured data</h3>
                  <span v-if="jsonLdTypesLabel" class="text-muted text-xs">{{ jsonLdTypesLabel }}</span>
                </div>
              </template>
              <!-- Raw JSON-LD source: a code block, which no Nuxt UI component renders. -->
              <pre
                v-for="(block, i) in jsonLdBlocks"
                :key="i"
                class="bg-muted text-default m-0 max-h-64 overflow-auto p-3 font-mono text-xs leading-relaxed whitespace-pre-wrap break-all"
              >{{ block }}</pre>
              <p v-if="!jsonLdBlocks.length" class="text-dimmed px-3 py-3 text-sm italic">Missing</p>
            </UCard>
          </div>
          <UEmpty
            v-else
            variant="naked"
            size="sm"
            icon="i-hugeicons-file-search"
            title="No meta loaded yet."
          />
        </div>
      </template>

      <template #issues>
        <ul class="m-0 flex list-none flex-col gap-2 px-3 pt-3 pb-3" data-testid="page-details-findings">
          <li v-if="findings.length === 0">
            <UEmpty
              variant="naked"
              size="sm"
              icon="i-hugeicons-checkmark-circle-02"
              title="No issues found on this page."
            />
          </li>
          <li v-for="f in findings" :key="`${f.rule}:${f.message}`" :data-rule="f.rule">
            <UAlert
              :color="f.severity === 'error' ? 'error' : 'warning'"
              variant="subtle"
              orientation="horizontal"
              :icon="f.severity === 'error' ? 'i-hugeicons-alert-circle' : 'i-hugeicons-alert-02'"
              :title="f.message"
              :description="f.detail || undefined"
              :aria-label="f.severity"
              :ui="{ root: 'px-2.5 py-2', title: 'text-default text-sm font-normal leading-snug', description: 'text-muted text-xs leading-snug' }"
            >
              <template #actions>
                <UButton
                  color="neutral"
                  variant="ghost"
                  size="xs"
                  icon="i-hugeicons-wrench-01"
                  label="Fix"
                  class="shrink-0"
                  :data-testid="`page-details-fix-${f.rule}`"
                  @click="emit('fix', fixPrompt(f))"
                />
              </template>
            </UAlert>
          </li>
        </ul>
      </template>
    </UTabs>
  </aside>
</template>
