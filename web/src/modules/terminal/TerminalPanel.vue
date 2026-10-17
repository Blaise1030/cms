<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal, type ITheme } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { Maximize2, Minimize2, PanelLeft, Plus } from "@lucide/vue";
import { useEventListener } from "@vueuse/core";
import { toast } from "vue-sonner";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import {
  Questionnaire,
  QuestionnaireActions,
  QuestionnaireChoice,
  QuestionnaireChoices,
  QuestionnaireItem,
  QuestionnaireSubmit,
  QuestionnaireTitle,
} from "@/components/ui/questionnaire";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { useDaemon } from "@/core/daemon";
import type { TerminalSession } from "@/core/types";
import { useTheme } from "@/lib/theme";
import { bracketedPaste, errorMessage } from "@workbench/utils";
import { useChatStore } from "@/modules/chat/store";
import { agentPrompt, isRequest, useAgentOutboxStore } from "./outbox";
import { buildTerminalUrl, connectTerminal, type TerminalConnection } from "./socket";
import { useTerminalStore } from "./store";

/** `active`: the panel is the agent tab in view, so it takes the outbox. */
const props = withDefaults(defineProps<{ projectId: string; active?: boolean }>(), { active: true });

const daemon = useDaemon();
const store = useTerminalStore();
const chats = useChatStore();
const outbox = useAgentOutboxStore();
const theme = useTheme();

/** xterm's default ANSI colors are tuned for dark backgrounds; yellow and white vanish on white. */
const LIGHT_ANSI: ITheme = {
  black: "#24292f",
  red: "#cf222e",
  green: "#116329",
  yellow: "#4d2d00",
  blue: "#0969da",
  magenta: "#8250df",
  cyan: "#1b7c83",
  white: "#6e7781",
  brightBlack: "#57606a",
  brightRed: "#a40e26",
  brightGreen: "#1a7f37",
  brightYellow: "#633c01",
  brightBlue: "#218bff",
  brightMagenta: "#a475f9",
  brightCyan: "#3192aa",
  brightWhite: "#8c959f",
};

const providerLabels: Record<string, string> = {
  cursor: "Cursor CLI",
  claude: "Claude",
  codex: "Codex",
};

function providerLabel(id: string): string {
  return providerLabels[id] ?? id;
}

const state = ref<"loading" | "idle" | "running">("loading");
const ended = ref<{ code: number | null } | null>(null);
const host = ref<HTMLElement | null>(null);

let term: Terminal | null = null;
let fit: FitAddon | null = null;
let conn: TerminalConnection | null = null;
let observer: ResizeObserver | null = null;
let muted = false;
/** The current socket is open and its replay is on screen, so pasted input reaches the agent. */
let ready = false;
let pending: Uint8Array[] = [];
let raf: number | null = null;
let retryTimer: ReturnType<typeof setTimeout> | null = null;
let retries = 0;
let runningProvider: string | null = null;
let disposed = false;
/** A just-started agent CLI drops input until its UI is up. */
const AGENT_BOOT_MS = 4000;
const SUBMIT_DELAY_MS = 150;
let bootUntil = 0;
let deliverTimer: ReturnType<typeof setTimeout> | null = null;

function cssColor(parent: HTMLElement, name: string, fallback: string): string {
  try {
    const probe = document.createElement("span");
    probe.style.color = `var(${name})`;
    parent.appendChild(probe);
    const value = getComputedStyle(probe).color;
    probe.remove();
    return value || fallback;
  } catch {
    return fallback;
  }
}

function terminalTheme(parent: HTMLElement): ITheme {
  const dark = theme.resolved.value === "dark";
  const foreground = cssColor(parent, "--foreground", dark ? "#fafafa" : "#111111");
  return {
    ...(dark ? {} : LIGHT_ANSI),
    background: cssColor(parent, "--background", dark ? "#0a0a0a" : "#ffffff"),
    foreground,
    cursor: foreground,
    selectionBackground: dark ? "#ffffff33" : "#0000001f",
  };
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
  ready = false;
  conn = connectTerminal({
    url: buildTerminalUrl(daemon.config.value, props.projectId),
    onReplay(bytes) {
      if (!term) return;
      pending = [];
      term.reset();
      muted = true;
      term.write(bytes, () => {
        muted = false;
        ready = true;
        deliver();
      });
      retries = 0;
      sendResize();
    },
    onData: queue,
    onExit(code) {
      endSession(code);
    },
    onClose() {
      conn = null;
      ready = false;
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
    theme: terminalTheme(host.value),
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

/** Stop reconnecting, but let the terminal socket deliver its ordered final output. */
function markEnded(code: number | null) {
  ended.value = { code };
  if (retryTimer) clearTimeout(retryTimer);
  retryTimer = null;
}

function endSession(code: number | null) {
  markEnded(code);
  conn?.close();
  conn = null;
  ready = false;
}

function teardown() {
  if (retryTimer) clearTimeout(retryTimer);
  retryTimer = null;
  ready = false;
  if (raf !== null) cancelAnimationFrame(raf);
  raf = null;
  pending = [];
  conn?.close();
  conn = null;
  disposeTerminal();
}

/** Start a session; with no `resume`, continue the chat tab's conversation when it has one to resume. */
async function start(resume?: TerminalSession) {
  ended.value = null;
  if (!resume) {
    await store.loadSessions(props.projectId);
    const chatId = chats.activeChatId(props.projectId);
    resume = store.sessions.find((s) => s.id === chatId && s.resumable);
  }
  if (!(await store.start(props.projectId, resume))) {
    state.value = "idle";
    return;
  }
  runningProvider = resume?.provider ?? store.provider;
  bootUntil = Date.now() + AGENT_BOOT_MS;
  teardown();
  state.value = "running";
  void store.loadSessions(props.projectId);
  await mountTerminal();
}

onMounted(async () => {
  store.sessionId = null;
  store.sessions = [];
  void store.loadSessions(props.projectId);
  await store.loadProviders();
  try {
    const status = await daemon.api.terminalStatus(props.projectId);
    if (status.running) {
      runningProvider = status.provider ?? store.provider;
      store.sessionId = status.sessionId ?? null;
      state.value = "running";
      await mountTerminal();
      return;
    }
  } catch {
    // fall through to the picker
  }
  state.value = "idle";
  startForRequests();
});

/** Set while stopping one session to open another; that stop's exit is expected. */
let switching = false;

const offExited = daemon.on("terminal.exited", (e) => {
  if (e.projectId !== props.projectId) return;
  void store.loadSessions(props.projectId);
  if (ended.value || switching) return;
  // A late exit of the session we just switched away from.
  if (e.sessionId && store.sessionId && e.sessionId !== store.sessionId) return;
  markEnded(e.code);
});

const offSession = daemon.on("terminal.session", (e) => {
  if (e.projectId === props.projectId) void store.loadSessions(props.projectId);
});

onBeforeUnmount(() => {
  offExited();
  offSession();
  disposed = true;
  if (deliverTimer) clearTimeout(deliverTimer);
  teardown();
});

// --- Session history ---------------------------------------------------------------------------

const sessionsOpen = ref(false);

watch(sessionsOpen, (open) => {
  if (open) void store.loadSessions(props.projectId);
});

const live = computed(() => state.value === "running" && !ended.value);

function isLive(s: TerminalSession): boolean {
  return live.value && s.id === store.sessionId;
}

function sessionTitle(s: TerminalSession): string {
  return s.title || `${providerLabel(s.provider)} session`;
}

const headerTitle = computed(() => {
  const current = store.sessions.find((s) => s.id === store.sessionId);
  if (current && state.value === "running") return sessionTitle(current);
  return state.value === "running" ? `${providerLabel(runningProvider ?? "")} session` : "New session";
});

function when(ms: number): string {
  return new Date(ms).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

/** Stop the live session (if any), then resume `session`, or show the picker for a new one. */
async function switchTo(session: TerminalSession | null) {
  sessionsOpen.value = false;
  if (session && isLive(session)) return;
  switching = true;
  try {
    const wasLive = live.value;
    teardown();
    if (wasLive) await daemon.api.stopTerminal(props.projectId);
    store.sessionId = null;
    ended.value = null;
    store.error = null;
    if (session) await start(session);
    else {
      chats.newChat(props.projectId);
      state.value = "idle";
    }
  } catch (e) {
    store.error = errorMessage(e);
    state.value = "idle";
  } finally {
    switching = false;
  }
}

// --- Floating window ---------------------------------------------------------------------------

/** Top-left corner of the floating window, kept so its header stays on screen. */
const floatPos = ref({ x: window.innerWidth - 460, y: 72 });
let drag: { pointerX: number; pointerY: number; x: number; y: number } | null = null;

function clampToViewport(x: number, y: number) {
  return {
    x: Math.min(Math.max(x, 0), window.innerWidth - 120),
    y: Math.min(Math.max(y, 0), window.innerHeight - 44),
  };
}

function onHeaderPointerDown(e: PointerEvent) {
  if (!store.floating || e.button !== 0 || (e.target as Element).closest("button")) return;
  drag = { pointerX: e.clientX, pointerY: e.clientY, ...floatPos.value };
  e.preventDefault();
}

useEventListener(window, "pointermove", (e: PointerEvent) => {
  if (!drag) return;
  floatPos.value = clampToViewport(drag.x + e.clientX - drag.pointerX, drag.y + e.clientY - drag.pointerY);
});
useEventListener(window, "pointerup", () => {
  drag = null;
});

const floatStyle = computed(() => ({ left: `${floatPos.value.x}px`, top: `${floatPos.value.y}px` }));

function notFoundCommand(): string {
  return store.providers.find((p) => p.id === runningProvider)?.command[0] ?? runningProvider ?? "command";
}

/** Paste everything waiting in the outbox, if the agent can take it now. */
function deliver() {
  if (!props.active || !ready || !conn || outbox.messages.length === 0) return;
  const wait = bootUntil - Date.now();
  if (wait > 0 && outbox.messages.some(isRequest)) {
    deliverTimer ??= setTimeout(() => {
      deliverTimer = null;
      deliver();
    }, wait);
    return;
  }
  const c = conn;
  for (const message of outbox.drain()) {
    c.send(bracketedPaste(agentPrompt(message)));
    // Enter as its own keystroke: sent with the paste, the agent's input may swallow it.
    if (isRequest(message)) setTimeout(() => c.send("\r"), SUBMIT_DELAY_MS);
  }
  term?.focus();
}

/** A request with no session running starts one with the chosen agent. */
function startForRequests() {
  if (props.active && state.value === "idle" && !store.starting && outbox.messages.some(isRequest)) void onStart();
}

watch([() => outbox.messages.length, () => props.active], () => {
  startForRequests();
  deliver();
});
watch(
  theme.resolved,
  () => {
    if (term && host.value) term.options.theme = terminalTheme(host.value);
  },
  { flush: "post" },
);

// --- Image drops -------------------------------------------------------------------------------

/** An image is being dragged over the panel. */
const dropping = ref(false);

function carriesFiles(e: DragEvent): boolean {
  return e.dataTransfer?.types.includes("Files") ?? false;
}

function onDragOver(e: DragEvent) {
  if (!carriesFiles(e)) return;
  e.preventDefault();
  dropping.value = true;
}

function onDragLeave(e: DragEvent) {
  if (!(e.currentTarget as Element).contains(e.relatedTarget as Node | null)) dropping.value = false;
}

/** Agent CLIs attach an image whose path is pasted, escaped the way a terminal escapes a dropped file. */
async function onDrop(e: DragEvent) {
  if (!carriesFiles(e)) return;
  e.preventDefault();
  dropping.value = false;
  const images = [...(e.dataTransfer?.files ?? [])].filter((f) => f.type.startsWith("image/"));
  if (images.length === 0) {
    toast.error("Only images can be dropped into the agent");
    return;
  }
  try {
    const paths = await Promise.all(images.map((f) => daemon.api.attachTerminalImage(props.projectId, f)));
    outbox.send({ kind: "text", text: paths.map((p) => p.replace(/ /g, "\\ ") + " ").join("") });
  } catch (err) {
    toast.error(`Couldn't attach image: ${errorMessage(err)}`);
  }
}

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
  <Teleport to="body" :disabled="!store.floating">
  <div
    data-testid="terminal-panel"
    :data-floating="store.floating || undefined"
    class="bg-background text-foreground flex min-h-0 flex-col"
    :class="
      store.floating
        ? 'fixed z-40 h-[min(560px,80vh)] w-[min(440px,90vw)] overflow-hidden rounded-lg border shadow-xl'
        : 'relative h-full'
    "
    :style="store.floating ? floatStyle : undefined"
    @dragover="onDragOver"
    @dragleave="onDragLeave"
    @drop="onDrop"
  >
    <div
      v-if="dropping"
      data-testid="terminal-drop-overlay"
      class="bg-background/80 border-primary text-foreground pointer-events-none absolute inset-0 z-10 flex items-center justify-center rounded-[inherit] border-2 border-dashed text-sm"
    >
      Drop images for the agent
    </div>
    <TooltipProvider>
      <div
        data-testid="terminal-header"
        class="flex h-11 shrink-0 items-center gap-1 border-b px-1.5"
        :class="{ 'cursor-move touch-none select-none': store.floating }"
        @pointerdown="onHeaderPointerDown"
      >
        <Popover v-model:open="sessionsOpen">
          <PopoverTrigger as-child>
            <Button
              :variant="sessionsOpen ? 'secondary' : 'ghost'"
              size="icon-sm"
              aria-label="Sessions"
              data-testid="terminal-sessions-toggle"
            >
              <PanelLeft />
            </Button>
          </PopoverTrigger>
          <PopoverContent align="start" class="flex max-h-96 w-72 flex-col overflow-y-auto p-1.5">
            <nav aria-label="Sessions" data-testid="terminal-sessions" class="flex flex-col">
              <p v-if="store.sessions.length === 0" class="text-muted-foreground p-2 text-sm">No sessions yet</p>
              <button
                v-for="s in store.sessions"
                :key="s.id"
                type="button"
                class="hover:bg-muted flex w-full flex-col items-start gap-0.5 rounded-md px-2 py-1.5 text-left text-sm disabled:pointer-events-none disabled:opacity-50"
                :class="{ 'bg-primary/10 ring-primary/40 hover:bg-primary/15 ring-1 ring-inset': s.id === store.sessionId }"
                :disabled="!isLive(s) && (!s.resumable || store.starting)"
                :title="isLive(s) || s.resumable ? undefined : 'This session can\'t be resumed'"
                :aria-current="s.id === store.sessionId ? 'true' : undefined"
                data-testid="terminal-session"
                @click="switchTo(s)"
              >
                <span class="w-full truncate" :class="{ 'font-medium': s.id === store.sessionId }">{{
                  sessionTitle(s)
                }}</span>
                <span class="text-muted-foreground text-xs">
                  {{ providerLabel(s.provider) }} · {{ when(s.updatedAt) }}<template v-if="isLive(s)"> · Running</template>
                </span>
              </button>
            </nav>
          </PopoverContent>
        </Popover>
        <span class="min-w-0 flex-1 truncate px-1 text-sm" data-testid="terminal-title">{{ headerTitle }}</span>
        <Tooltip>
          <TooltipTrigger as-child>
            <Button
              variant="ghost"
              size="icon-sm"
              :aria-label="store.floating ? 'Dock' : 'Pop out'"
              data-testid="terminal-float-toggle"
              @click="store.floating = !store.floating"
            >
              <Minimize2 v-if="store.floating" />
              <Maximize2 v-else />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{{ store.floating ? "Dock" : "Pop out" }}</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger as-child>
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label="New session"
              data-testid="terminal-new-session"
              :disabled="state === 'loading' || store.starting"
              @click="switchTo(null)"
            >
              <Plus />
            </Button>
          </TooltipTrigger>
          <TooltipContent>New session</TooltipContent>
        </Tooltip>
      </div>
    </TooltipProvider>
    <div v-if="state === 'running'" ref="host" class="min-h-0 flex-1 p-2" />
    <div
      v-else-if="state === 'idle'"
      class="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-sm"
    >
      <Questionnaire class="max-w-sm" @submit.prevent="onStart">
        <QuestionnaireItem name="provider" required data-testid="terminal-provider">
          <QuestionnaireTitle>Which agent should work on this project?</QuestionnaireTitle>
          <QuestionnaireChoices>
            <QuestionnaireChoice
              v-for="p in store.providers"
              :key="p.id"
              :value="p.id"
              :checked="store.provider === p.id"
              @update:checked="$event && store.setProvider(p.id)"
            >
              {{ providerLabel(p.id) }}
            </QuestionnaireChoice>
          </QuestionnaireChoices>
        </QuestionnaireItem>
        <QuestionnaireActions>
          <QuestionnaireSubmit data-testid="terminal-start" :disabled="store.starting">Start</QuestionnaireSubmit>
        </QuestionnaireActions>
      </Questionnaire>
      <p v-if="store.error" data-testid="terminal-error" class="text-destructive max-w-sm text-center">
        {{ store.error }}
      </p>
    </div>
    <div
      v-if="ended"
      data-testid="terminal-ended"
      class="border-border bg-background flex shrink-0 flex-wrap items-center justify-between gap-3 border-t px-3 py-2 text-sm"
    >
      <p v-if="ended.code === 127">`{{ notFoundCommand() }}` not found on PATH</p>
      <p v-else>Session ended (code {{ ended.code ?? "unknown" }})</p>
      <Button data-testid="terminal-restart" @click="onStart">Restart</Button>
    </div>
  </div>
  </Teleport>
</template>
