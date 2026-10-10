<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { Button } from "@/components/ui/button";
import { useDaemon } from "@/core/daemon";
import { errorMessage } from "@/lib/errors";
import { elementPrompt } from "@/modules/preview/elementPicker";
import { usePreviewStore } from "@/modules/preview/store";
import { bracketedPaste } from "./paste";
import { buildTerminalUrl, connectTerminal, type TerminalConnection } from "./socket";
import { useTerminalStore } from "./store";

const props = defineProps<{ projectId: string; autoApprove?: boolean }>();
const emit = defineEmits<{ toggleAutoApprove: [] }>();

const daemon = useDaemon();
const store = useTerminalStore();
const preview = usePreviewStore();

const state = ref<"loading" | "idle" | "running">("loading");
const ended = ref<{ code: number | null } | null>(null);
const host = ref<HTMLElement | null>(null);

let term: Terminal | null = null;
let fit: FitAddon | null = null;
let conn: TerminalConnection | null = null;
let observer: ResizeObserver | null = null;
let muted = false;
let pending: Uint8Array[] = [];
let raf: number | null = null;
let retryTimer: ReturnType<typeof setTimeout> | null = null;
let retries = 0;
let runningProvider: string | null = null;
let disposed = false;

function cssColor(name: string, fallback: string): string {
  try {
    const probe = document.createElement("span");
    probe.style.color = `var(${name})`;
    document.body.appendChild(probe);
    const value = getComputedStyle(probe).color;
    probe.remove();
    return value || fallback;
  } catch {
    return fallback;
  }
}

function flush() {
  raf = null;
  if (!term || pending.length === 0) return;
  const total = pending.reduce((n, c) => n + c.length, 0);
  const merged = new Uint8Array(total);
  let off = 0;
  for (const c of pending) {
    merged.set(c, off);
    off += c.length;
  }
  pending = [];
  term.write(merged);
}

function queue(bytes: Uint8Array) {
  pending.push(bytes);
  if (raf === null) raf = requestAnimationFrame(flush);
}

function sendResize() {
  if (!term || !fit) return;
  fit.fit();
  conn?.resize(term.cols, term.rows);
}

function connect() {
  if (disposed) return;
  conn?.close();
  conn = connectTerminal({
    url: buildTerminalUrl(daemon.config.value, props.projectId),
    onReplay(bytes) {
      if (!term) return;
      pending = [];
      term.reset();
      muted = true;
      term.write(bytes, () => {
        muted = false;
      });
      retries = 0;
      sendResize();
    },
    onData: queue,
    onExit(code) {
      ended.value = { code };
      conn?.close();
      conn = null;
    },
    onClose() {
      conn = null;
      if (ended.value || disposed) return;
      const delay = Math.min(500 * 2 ** retries, 10_000);
      retries += 1;
      retryTimer = setTimeout(connect, delay);
    },
  });
}

async function mountTerminal() {
  await nextTick();
  try {
    await document.fonts?.ready;
  } catch {
    // fonts API unavailable; open anyway
  }
  if (disposed || !host.value || state.value !== "running") return;
  disposeTerminal();
  term = new Terminal({
    cursorBlink: true,
    fontSize: 13,
    fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, "Liberation Mono", monospace',
    scrollback: 5000,
    theme: {
      background: cssColor("--background", "#ffffff"),
      foreground: cssColor("--foreground", "#111111"),
      cursor: cssColor("--foreground", "#111111"),
    },
  });
  fit = new FitAddon();
  term.loadAddon(fit);
  term.open(host.value);
  term.onData((data) => {
    if (!muted) conn?.send(data);
  });
  observer = new ResizeObserver(() => sendResize());
  observer.observe(host.value);
  fit.fit();
  connect();
}

function disposeTerminal() {
  observer?.disconnect();
  observer = null;
  term?.dispose();
  term = null;
  fit = null;
}

function teardown() {
  if (retryTimer) clearTimeout(retryTimer);
  retryTimer = null;
  if (raf !== null) cancelAnimationFrame(raf);
  raf = null;
  pending = [];
  conn?.close();
  conn = null;
  disposeTerminal();
}

async function start() {
  ended.value = null;
  if (!(await store.start(props.projectId))) {
    state.value = "idle";
    return;
  }
  runningProvider = store.provider;
  teardown();
  state.value = "running";
  await mountTerminal();
}

onMounted(async () => {
  await store.loadProviders();
  try {
    const status = await daemon.api.terminalStatus(props.projectId);
    if (status.running) {
      runningProvider = status.provider ?? store.provider;
      state.value = "running";
      await mountTerminal();
      return;
    }
  } catch {
    // fall through to the picker
  }
  state.value = "idle";
});

onBeforeUnmount(() => {
  disposed = true;
  teardown();
});

function notFoundCommand(): string {
  return store.providers.find((p) => p.id === runningProvider)?.command[0] ?? runningProvider ?? "command";
}

watch(
  () => preview.picked,
  (pick) => {
    if (!pick || state.value !== "running" || !conn) return;
    const taken = preview.takePicked();
    if (!taken) return;
    conn.send(bracketedPaste(elementPrompt(taken)));
    term?.focus();
  },
);

async function onStart() {
  try {
    await start();
  } catch (e) {
    store.error = errorMessage(e);
    state.value = "idle";
  }
}
</script>

<template>
  <div data-testid="terminal-panel" class="bg-background relative flex h-full min-h-0 flex-col">
    <div v-if="state === 'running'" ref="host" class="min-h-0 flex-1 p-2" />
    <div
      v-else-if="state === 'idle'"
      class="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-sm"
    >
      <label class="flex items-center gap-2">
        <span class="text-muted-foreground">Agent</span>
        <select
          data-testid="terminal-provider"
          class="border-input bg-background rounded-md border px-2 py-1"
          :value="store.provider ?? ''"
          @change="store.setProvider(($event.target as HTMLSelectElement).value)"
        >
          <option v-for="p in store.providers" :key="p.id" :value="p.id">{{ p.id }}</option>
        </select>
      </label>
      <label class="flex items-center gap-2">
        <input
          type="checkbox"
          data-testid="auto-approve"
          :checked="autoApprove"
          @change="emit('toggleAutoApprove')"
        />
        <span>Auto-approve (applies on next start)</span>
      </label>
      <Button data-testid="terminal-start" :disabled="store.starting" @click="onStart">
        Start
      </Button>
      <p v-if="store.error" data-testid="terminal-error" class="text-destructive max-w-sm text-center">
        {{ store.error }}
      </p>
    </div>
    <div
      v-if="ended"
      data-testid="terminal-ended"
      class="bg-background/90 absolute inset-0 flex flex-col items-center justify-center gap-3 text-sm"
    >
      <p v-if="ended.code === 127">`{{ notFoundCommand() }}` not found on PATH</p>
      <p v-else>Session ended (code {{ ended.code ?? "unknown" }})</p>
      <Button data-testid="terminal-restart" @click="onStart">Restart</Button>
    </div>
  </div>
</template>
