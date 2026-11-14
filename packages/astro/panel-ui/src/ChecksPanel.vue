<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref } from "vue";
import Button from "./ui/Button.vue";
import Badge from "./ui/Badge.vue";
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
    class="orso-checks-root pointer-events-auto flex flex-col items-end gap-2"
    :class="{ 'pointer-events-none opacity-35': picking }"
    data-testid="orso-checks-panel"
  >
    <div
      v-show="open"
      class="bg-card text-card-foreground border-border flex w-[min(22rem,calc(100vw-24px))] max-h-[min(26rem,calc(100vh-80px))] flex-col gap-2.5 overflow-hidden rounded-lg border p-3 shadow-lg"
      data-orso-checks-drawer
    >
      <div class="flex items-center justify-between gap-2">
        <div class="text-sm font-medium tracking-tight">Preview checks</div>
        <Button variant="outline" size="xs" data-orso-checks-refresh @click="refresh">
          Refresh
        </Button>
      </div>
      <div class="text-muted-foreground text-[0.72rem]" data-orso-checks-summary>
        {{ summary }}
      </div>
      <ul class="m-0 flex list-none flex-col gap-1.5 overflow-auto p-0" data-orso-checks-findings>
        <li v-if="sorted.length === 0" class="text-muted-foreground px-0.5 py-1 text-sm">
          No issues found on this page.
        </li>
        <li
          v-for="f in sorted"
          :key="`${f.rule}:${f.message}`"
          class="border-border bg-muted/65 flex items-start gap-2 rounded-md border px-2.5 py-2"
          :data-rule="f.rule"
        >
          <Badge
            :variant="f.severity === 'error' ? 'destructive' : 'secondary'"
            class="mt-0.5 shrink-0 uppercase"
          >
            {{ f.severity }}
          </Badge>
          <div class="min-w-0 flex-1 text-sm">
            <div>{{ f.message }}</div>
            <div v-if="f.detail" class="text-muted-foreground mt-0.5 text-[0.72rem]">
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
        </li>
      </ul>
    </div>

    <Button
      variant="outline"
      size="sm"
      class="rounded-full shadow-lg"
      data-orso-checks-trigger
      :aria-expanded="open"
      @click="toggle"
    >
      Checks
    </Button>
  </div>
</template>
