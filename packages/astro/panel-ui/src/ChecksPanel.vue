<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref } from "vue";
import { CircleAlert, TriangleAlert } from "@lucide/vue";
import Button from "./ui/Button.vue";
import Badge from "./ui/Badge.vue";
import Card from "./ui/Card.vue";
import { sortFindings } from "../../src/checks/findings.js";
import { auditPage } from "../../src/checks/page.js";

type Finding = {
  rule: string;
  severity: "error" | "warning";
  page: string;
  message: string;
  detail?: string;
  count?: number;
};

const PICKER_ID = "__orso-picker";
const BRIDGE_TYPE = "orso:preview";

const open = ref(false);
const ranOnce = ref(false);
const picking = ref(false);
const findings = ref<Finding[]>([]);
const summary = ref("");

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
  if (open.value) runAudit();
}

function applyPickState() {
  const on = document.getElementById(PICKER_ID) !== null;
  picking.value = on;
  if (on && open.value) setOpen(false);
}

let observer: MutationObserver | null = null;
onMounted(() => {
  observer = new MutationObserver(applyPickState);
  observer.observe(document.documentElement, { childList: true, subtree: true });
  applyPickState();
});
onBeforeUnmount(() => observer?.disconnect());

const sorted = computed(() => findings.value);
</script>

<template>
  <div
    class="pointer-events-auto flex flex-col items-end gap-2"
    :class="{ 'pointer-events-none opacity-35': picking }"
    data-testid="orso-checks-panel"
  >
    <Card
      v-show="open"
      class="border-border w-[min(22rem,calc(100vw-24px))] max-h-[min(26rem,calc(100vh-80px))] gap-3 overflow-hidden rounded-xl border py-3 shadow-lg"
      data-orso-checks-drawer
    >
      <div class="flex items-center justify-between gap-2 px-3">
        <div class="text-sm font-medium tracking-tight">Preview checks</div>
        <Button variant="outline" size="xs" data-orso-checks-refresh @click="refresh">
          Refresh
        </Button>
      </div>
      <div class="text-muted-foreground px-3 text-[0.72rem]" data-orso-checks-summary>
        {{ summary }}
      </div>
      <ul class="m-0 flex list-none flex-col gap-2 overflow-auto px-3 pb-0.5" data-orso-checks-findings>
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
                aria-hidden="true"
              >
                <CircleAlert v-if="f.severity === 'error'" class="size-3.5" />
                <TriangleAlert v-else class="size-3.5" />
              </div>
              <div class="min-w-0 flex-1">
                <div class="mb-1 flex items-center gap-1.5">
                  <Badge
                    :variant="f.severity === 'error' ? 'destructive' : 'secondary'"
                    class="uppercase"
                  >
                    {{ f.severity }}
                  </Badge>
                </div>
                <div class="text-sm leading-snug">{{ f.message }}</div>
                <div v-if="f.detail" class="text-muted-foreground mt-1 text-[0.72rem]">
                  {{ f.detail }}
                </div>
              </div>
              <Button
                size="xs"
                class="shrink-0 self-center"
                :data-orso-checks-fix="f.rule"
                @click="postFix(fixPrompt(f))"
              >
                Fix this
              </Button>
            </div>
          </Card>
        </li>
      </ul>
    </Card>

    <Button
      variant="outline"
      size="sm"
      class="rounded-full shadow-lg"
      data-orso-checks-trigger
      :aria-expanded="open"
      @click="toggle"
    >
      {{ open ? "Close checks" : "Preview checks" }}
    </Button>
  </div>
</template>
