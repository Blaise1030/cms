<script setup lang="ts">
import { computed, ref } from "vue";
import {
  ChevronsRight,
  CircleAlert,
  CodeXml,
  FileCode2,
  ImageIcon,
  Palette,
  RefreshCw,
  TriangleAlert,
  Wrench,
} from "@lucide/vue";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
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
</script>

<template>
  <aside
    class="bg-background flex h-full min-h-0 w-full min-w-0 flex-col"
    data-testid="page-details-sidebar"
  >
    <Tabs
      :model-value="tab"
      class="flex min-h-0 flex-1 flex-col"
      @update:model-value="tab = $event as Tab"
    >
      <header class="bg-background relative z-10 shrink-0 px-3 pt-2 pb-2">
        <div
          class="pointer-events-none absolute inset-x-0 top-full h-8 bg-linear-to-b from-background to-transparent"
          aria-hidden="true"
        />
        <div class="flex items-center justify-between gap-2 pb-2">
          <div class="text-sm font-medium tracking-tight">Page details</div>
          <div class="flex items-center gap-1">
            <Button
              variant="outline"
              size="xs"
              data-testid="page-details-refresh"
              @click="emit('refresh')"
            >
              <RefreshCw class="size-3" aria-hidden="true" />
              Refresh
            </Button>
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label="Close page details"
              data-testid="page-details-close"
              @click="emit('close')"
            >
              <ChevronsRight class="size-3.5" />
            </Button>
          </div>
        </div>
        <div
          class="text-muted-foreground flex flex-wrap items-center gap-x-3 gap-y-1 pb-2 text-[0.72rem]"
          data-testid="page-details-summary"
        >
          <template v-if="resources">
            <span class="inline-flex items-center gap-1" title="Scripts">
              <CodeXml class="size-3 shrink-0" aria-hidden="true" />
              <span class="tabular-nums">{{ resources.scripts }}</span>
            </span>
            <span class="inline-flex items-center gap-1" title="Styles">
              <Palette class="size-3 shrink-0" aria-hidden="true" />
              <span class="tabular-nums">{{ resources.styles }}</span>
            </span>
            <span class="inline-flex items-center gap-1" title="Images">
              <ImageIcon class="size-3 shrink-0" aria-hidden="true" />
              <span class="tabular-nums">{{ resources.images }}</span>
            </span>
            <span class="inline-flex items-center gap-1" title="HTML size">
              <FileCode2 class="size-3 shrink-0" aria-hidden="true" />
              <span class="tabular-nums">{{ formatBytes(resources.htmlBytes) }}</span>
            </span>
          </template>
          <span v-else>Open a page to inspect SEO fields.</span>
        </div>
        <TabsList class="border-border w-full border">
          <TabsTrigger value="meta" class="flex-1" data-testid="page-details-tab-meta">
            Meta
          </TabsTrigger>
          <TabsTrigger value="issues" class="flex-1" data-testid="page-details-tab-issues">
            Issues
            <span
              v-if="issueCount > 0"
              class="text-muted-foreground ml-1 tabular-nums"
            >
              {{ issueCount }}
            </span>
          </TabsTrigger>
        </TabsList>
      </header>

      <TabsContent
        value="meta"
        class="mt-0 min-h-0 flex-1 overflow-auto"
        data-testid="page-details-meta"
      >
        <div v-if="metaGroups.length" class="flex flex-col gap-5 px-3 pt-3 pb-3">
          <section class="flex flex-col gap-3">
            <h3 class="text-foreground text-xs font-semibold tracking-tight">Branding</h3>
            <div class="border-border divide-border divide-y rounded-lg border">
              <div class="grid gap-2 px-3 py-3">
                <div class="text-muted-foreground text-xs font-medium">Favicon</div>
                <div v-if="faviconUrl" class="flex items-center gap-3">
                  <img
                    :src="faviconSrc ?? faviconUrl"
                    alt=""
                    class="border-border bg-background size-8 rounded-md border object-contain p-1"
                    data-testid="page-details-favicon"
                  />
                  <span class="text-muted-foreground font-mono text-xs break-all" :title="faviconUrl">
                    {{ faviconUrl }}
                  </span>
                </div>
                <p v-else class="text-muted-foreground text-sm italic">Missing</p>
              </div>
              <div class="grid gap-2 px-3 py-3">
                <div class="text-muted-foreground text-xs font-medium">OG image</div>
                <div v-if="ogImageUrl" class="grid gap-2">
                  <img
                    :src="ogImageSrc ?? ogImageUrl"
                    alt=""
                    class="border-border bg-muted/40 max-h-40 w-full rounded-md border object-cover"
                    data-testid="page-details-og-image"
                  />
                  <span class="text-muted-foreground font-mono text-xs break-all" :title="ogImageUrl">
                    {{ ogImageUrl }}
                  </span>
                </div>
                <p v-else class="text-muted-foreground text-sm italic">Missing</p>
              </div>
            </div>
          </section>

          <section v-for="group in metaGroups" :key="group.id" class="flex flex-col gap-3">
            <h3 class="text-foreground text-xs font-semibold tracking-tight">
              {{ group.title }}
            </h3>
            <dl class="border-border divide-border m-0 divide-y rounded-lg border">
              <div
                v-for="row in group.rows"
                :key="row.key"
                class="grid gap-1.5 px-3 py-3"
              >
                <div class="flex items-baseline justify-between gap-3">
                  <dt class="text-muted-foreground text-xs font-medium">
                    {{ row.label }}
                  </dt>
                  <Badge
                    v-if="row.hint"
                    variant="secondary"
                    class="tabular-nums"
                  >
                    {{ row.hint }}
                  </Badge>
                </div>
                <dd
                  class="text-foreground m-0 text-sm leading-relaxed"
                  :class="[
                    row.value ? '' : 'text-muted-foreground italic',
                    row.mono ? 'font-mono text-xs break-all' : 'break-words',
                  ]"
                  :title="row.value ?? undefined"
                >
                  {{ row.value ?? "Missing" }}
                </dd>
              </div>
            </dl>
          </section>

          <section class="flex flex-col gap-3" data-testid="page-details-jsonld">
            <div class="flex items-baseline justify-between gap-3">
              <h3 class="text-foreground text-xs font-semibold tracking-tight">
                Structured data
              </h3>
              <span v-if="jsonLdTypesLabel" class="text-muted-foreground text-xs">
                {{ jsonLdTypesLabel }}
              </span>
            </div>
            <div v-if="jsonLdBlocks.length" class="flex flex-col gap-3">
              <pre
                v-for="(block, i) in jsonLdBlocks"
                :key="i"
                class="border-border bg-muted/40 text-foreground m-0 max-h-64 overflow-auto rounded-lg border p-3 font-mono text-[0.7rem] leading-relaxed whitespace-pre-wrap break-all"
              >{{ block }}</pre>
            </div>
            <p v-else class="text-muted-foreground text-sm italic">Missing</p>
          </section>
        </div>
        <p v-else class="text-muted-foreground px-3 pt-3 pb-3 text-sm">No meta loaded yet.</p>
      </TabsContent>

      <TabsContent
        value="issues"
        class="mt-0 min-h-0 flex-1 overflow-auto"
        data-testid="page-details-findings"
      >
        <ul class="m-0 flex list-none flex-col gap-2 px-3 pt-3 pb-3">
          <li v-if="findings.length === 0" class="text-muted-foreground px-0.5 py-2 text-sm">
            No issues found on this page.
          </li>
          <li
            v-for="f in findings"
            :key="`${f.rule}:${f.message}`"
            class="border-border bg-card flex items-start gap-2 rounded-lg border px-2.5 py-2"
            :data-rule="f.rule"
          >
            <div
              class="mt-0.5 flex size-6 shrink-0 items-center justify-center rounded-md border"
              :class="
                f.severity === 'error'
                  ? 'border-destructive/30 bg-destructive/15 text-destructive'
                  : 'border-amber-500/35 bg-amber-500/15 text-amber-600 dark:text-amber-400'
              "
              :aria-label="f.severity"
            >
              <CircleAlert v-if="f.severity === 'error'" class="size-3.5" />
              <TriangleAlert v-else class="size-3.5" />
            </div>
            <div class="min-w-0 flex-1">
              <div class="text-sm leading-snug">{{ f.message }}</div>
              <div v-if="f.detail" class="text-muted-foreground mt-0.5 text-xs leading-snug">
                {{ f.detail }}
              </div>
            </div>
            <Button
              size="xs"
              variant="ghost"
              class="text-muted-foreground hover:text-foreground h-6 shrink-0 px-1.5"
              :data-testid="`page-details-fix-${f.rule}`"
              @click="emit('fix', fixPrompt(f))"
            >
              <Wrench class="size-3" aria-hidden="true" />
              Fix
            </Button>
          </li>
        </ul>
      </TabsContent>
    </Tabs>
  </aside>
</template>
