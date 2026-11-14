<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref } from "vue";
import { Check, CircleAlert, RefreshCw, TriangleAlert, Wrench, X } from "@lucide/vue";
import Button from "./ui/Button.vue";
import Card from "./ui/Card.vue";
import { sortFindings } from "../../src/checks/findings.js";
import { DESCRIPTION_MAX, TITLE_MAX, readPageMeta } from "../../src/checks/meta.js";
import { auditPage } from "../../src/checks/page.js";

type Finding = {
  rule: string;
  severity: "error" | "warning";
  page: string;
  message: string;
  detail?: string;
  count?: number;
};

type PageMeta = ReturnType<typeof readPageMeta>;

const PICKER_ID = "__orso-picker";
const BRIDGE_TYPE = "orso:preview";

const open = ref(false);
const ranOnce = ref(false);
const picking = ref(false);
const findings = ref<Finding[]>([]);
const summary = ref("");
const meta = ref<PageMeta | null>(null);

function pagePath() {
  const path = `${location.pathname}${location.search}`;
  return path === "" ? "/" : path;
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function resourceSummary(doc: Document) {
  return {
    scripts: doc.querySelectorAll("script").length,
    styles: doc.querySelectorAll('link[rel="stylesheet"], style').length,
    images: doc.querySelectorAll("img").length,
    htmlBytes: new TextEncoder().encode(doc.documentElement.outerHTML).length,
  };
}

function fixPrompt(finding: Finding) {
  const path = finding.page || pagePath();
  const lines = [`The preview checks found an issue on ${path}: ${finding.message}`];
  if (finding.detail) lines.push(finding.detail);
  lines.push(
    "",
    "Fix it in the site and keep the existing copy and design.",
    "Rely on HMR; do not run a build or publish.",
  );
  return lines.join("\n");
}

function postFix(text: string) {
  if (window.parent === window) return;
  window.parent.postMessage({ type: BRIDGE_TYPE, op: "fix", text }, "*");
}

function runAudit() {
  const next = auditPage(document, { path: pagePath() });
  findings.value = sortFindings(next);
  meta.value = readPageMeta(document);
  const { scripts, styles, images, htmlBytes } = resourceSummary(document);
  summary.value = `${scripts} scripts · ${styles} styles · ${images} images · HTML ${formatBytes(htmlBytes)}`;
  ranOnce.value = true;
}

function setOpen(next: boolean) {
  open.value = next;
  if (next && !ranOnce.value) runAudit();
}

function toggle() {
  setOpen(!open.value);
}

function refresh() {
  runAudit();
}

function applyPickState() {
  const on = document.getElementById(PICKER_ID) !== null;
  picking.value = on;
  if (on && open.value) setOpen(false);
}

function display(value: string) {
  return value.trim() ? value : null;
}

function h1Label(m: PageMeta) {
  if (m.h1Count === 0) return null;
  if (m.h1Count === 1) return m.h1;
  return `${m.h1} (${m.h1Count} H1s)`;
}

let observer: MutationObserver | null = null;
onMounted(() => {
  observer = new MutationObserver(applyPickState);
  observer.observe(document.documentElement, { childList: true, subtree: true });
  applyPickState();
  runAudit();
});
onBeforeUnmount(() => observer?.disconnect());

const sorted = computed(() => findings.value);
const errorCount = computed(() => findings.value.filter((f) => f.severity === "error").length);
const warningCount = computed(() => findings.value.filter((f) => f.severity === "warning").length);
const triggerLabel = computed(() => {
  if (!ranOnce.value) return "Page details";
  const parts: string[] = [];
  if (errorCount.value) parts.push(`${errorCount.value} error${errorCount.value === 1 ? "" : "s"}`);
  if (warningCount.value) {
    parts.push(`${warningCount.value} warning${warningCount.value === 1 ? "" : "s"}`);
  }
  if (parts.length === 0) return "Page details";
  return parts.join(", ");
});

const metaRows = computed(() => {
  const m = meta.value;
  if (!m) return [];
  return [
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
    { key: "canonical", label: "Canonical", value: display(m.canonical) },
    { key: "robots", label: "Robots", value: display(m.robots) },
    { key: "lang", label: "Language", value: display(m.lang) },
    { key: "h1", label: "H1", value: h1Label(m) },
    { key: "ogTitle", label: "OG title", value: display(m.ogTitle) },
    { key: "ogDescription", label: "OG description", value: display(m.ogDescription) },
    { key: "ogImage", label: "OG image", value: display(m.ogImage) },
    {
      key: "jsonLd",
      label: "Structured data",
      value: m.jsonLdTypes.length > 0 ? m.jsonLdTypes.join(", ") : null,
    },
  ];
});
</script>

<template>
  <div
    class="pointer-events-auto flex flex-col items-end gap-2"
    :class="{ 'pointer-events-none opacity-35': picking }"
    data-testid="orso-checks-panel"
  >
    <Card
      v-show="open"
      class="border-border flex w-[min(22rem,calc(100vw-24px))] max-h-[min(32rem,calc(100vh-80px))] flex-col gap-3 overflow-hidden rounded-xl border py-3 shadow-lg"
      data-orso-checks-drawer
    >
      <div class="flex shrink-0 items-center justify-between gap-2 px-3">
        <div class="text-sm font-medium tracking-tight">Page details</div>
        <Button variant="outline" size="xs" data-orso-checks-refresh @click="refresh">
          <RefreshCw class="size-3" aria-hidden="true" />
          Refresh
        </Button>
      </div>
      <div class="text-muted-foreground shrink-0 px-3 text-[0.72rem]" data-orso-checks-summary>
        {{ summary }}
      </div>
      <div class="flex min-h-0 flex-1 flex-col gap-3 overflow-auto px-3 pb-0.5">
        <dl
          v-if="metaRows.length"
          class="border-border m-0 flex list-none flex-col gap-2 border-b pb-3"
          data-orso-checks-meta
        >
          <div v-for="row in metaRows" :key="row.key" class="grid gap-0.5">
            <div class="flex items-baseline justify-between gap-2">
              <dt class="text-muted-foreground text-[0.68rem] font-medium tracking-wide uppercase">
                {{ row.label }}
              </dt>
              <span v-if="row.hint" class="text-muted-foreground text-[0.68rem] tabular-nums">
                {{ row.hint }}
              </span>
            </div>
            <dd
              class="m-0 text-sm leading-snug break-words"
              :class="row.value ? '' : 'text-muted-foreground'"
            >
              {{ row.value ?? "Missing" }}
            </dd>
          </div>
        </dl>
        <ul class="m-0 flex list-none flex-col gap-2" data-orso-checks-findings>
          <li v-if="sorted.length === 0" class="text-muted-foreground px-0.5 py-1 text-sm">
            No issues found on this page.
          </li>
          <li v-for="f in sorted" :key="`${f.rule}:${f.message}`" :data-rule="f.rule">
            <Card
              class="border-border bg-card gap-0 rounded-lg border py-0 shadow-sm"
            >
              <div class="flex items-start gap-2.5 px-3 py-2.5">
                <div
                  class="mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-md border"
                  :class="
                    f.severity === 'error'
                      ? 'border-destructive/30 bg-destructive/15 text-destructive'
                      : 'border-[color-mix(in_oklch,var(--warning)_35%,transparent)] bg-[color-mix(in_oklch,var(--warning)_18%,transparent)] text-[color-mix(in_oklch,var(--warning)_90%,white)]'
                  "
                  :aria-label="f.severity"
                >
                  <CircleAlert v-if="f.severity === 'error'" class="size-3.5" />
                  <TriangleAlert v-else class="size-3.5" />
                </div>
                <div class="min-w-0 flex-1">
                  <div class="text-sm leading-snug">{{ f.message }}</div>
                  <div v-if="f.detail" class="text-muted-foreground mt-1 text-[0.72rem]">
                    {{ f.detail }}
                  </div>
                  <Button
                    size="xs"
                    class="mt-2"
                    :data-orso-checks-fix="f.rule"
                    @click="postFix(fixPrompt(f))"
                  >
                    <Wrench class="size-3" aria-hidden="true" />
                    Fix this
                  </Button>
                </div>
              </div>
            </Card>
          </li>
        </ul>
      </div>
    </Card>

    <Button
      variant="outline"
      size="sm"
      class="rounded-full shadow-lg"
      data-orso-checks-trigger
      :aria-expanded="open"
      :aria-label="open ? 'Close' : triggerLabel"
      @click="toggle"
    >
      <template v-if="open">
        <X class="size-3.5" aria-hidden="true" />
        Close
      </template>
      <template v-else-if="!ranOnce">
        Page details
      </template>
      <template v-else-if="errorCount === 0 && warningCount === 0">
        <Check class="size-3.5 text-emerald-500" aria-hidden="true" />
        Page details
      </template>
      <template v-else>
        <span
          v-if="errorCount > 0"
          class="inline-flex items-center gap-1 text-destructive"
        >
          <CircleAlert class="size-3.5" aria-hidden="true" />
          {{ errorCount }}
        </span>
        <span
          v-if="warningCount > 0"
          class="inline-flex items-center gap-1 text-[color-mix(in_oklch,var(--warning)_90%,white)]"
        >
          <TriangleAlert class="size-3.5" aria-hidden="true" />
          {{ warningCount }}
        </span>
      </template>
    </Button>
  </div>
</template>
