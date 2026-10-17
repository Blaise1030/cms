<script setup lang="ts">
import { computed, onBeforeUnmount, ref, shallowRef, watch } from "vue";
import { Chat } from "@ai-sdk/vue";
import { DefaultChatTransport, type UIMessage } from "ai";
import { CircleAlert, LoaderCircle, MessageSquare, Plus } from "@lucide/vue";
import { toast } from "vue-sonner";
import { errorMessage } from "@workbench/utils";
import { mapErrorResponse } from "@/core/api";
import { useDaemon } from "@/core/daemon";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import {
  MessageScroller,
  MessageScrollerButton,
  MessageScrollerContent,
  MessageScrollerItem,
  MessageScrollerProvider,
  MessageScrollerViewport,
} from "@/components/ui/message-scroller";
import { useTerminalStore } from "@/modules/terminal/store";
import ChatComposer from "./ChatComposer.vue";
import ChatHistory from "./ChatHistory.vue";
import ChatMessage from "./ChatMessage.vue";
import { expandReferences, referenceTag } from "./references";
import { useChatStore } from "./store";

const props = defineProps<{ projectId: string; active: boolean }>();
defineEmits<{ "open-terminal": [] }>();

/** Terminal providers the daemon can also drive headlessly. */
const CHAT_PROVIDERS = ["cursor", "claude", "codex"];
const SUGGESTIONS = [
  "Write a new blog post about our latest update",
  "Make the homepage headline punchier",
  "Fix typos on the About page",
];

const daemon = useDaemon();
const store = useChatStore();
const terminal = useTerminalStore();

const chat = shallowRef<Chat<UIMessage> | null>(null);
const input = ref("");
const composer = ref<InstanceType<typeof ChatComposer> | null>(null);
const historyOpen = ref(false);
const terminalRunning = ref(false);
/** Until the first terminal status arrives, `terminalRunning` may be stale. */
const terminalChecked = ref(false);
/** A `send` is waiting for the history or for its turn to end. */
const sending = ref(false);
/** The chat's own provider, or the user's pick; null falls back to the terminal's. */
const chosen = ref<string | null>(null);
/** The latest `open()`; sends wait for it so none land while the history loads. */
let opened: Promise<void> = Promise.resolve();
/** What each `[Element #n]` in the input stands for, until the input is sent. */
const references = new Map<string, string>();

const providers = computed(() => terminal.providers.map((p) => p.id).filter((id) => CHAT_PROVIDERS.includes(id)));
const provider = computed({
  get: () =>
    chosen.value ??
    (terminal.provider && providers.value.includes(terminal.provider) ? terminal.provider : providers.value[0]) ??
    null,
  set: (id) => (chosen.value = id),
});
const messages = computed(() => chat.value?.messages ?? []);
const streaming = computed(() => chat.value?.status === "submitted" || chat.value?.status === "streaming");
/** A new turn would be refused (or race the current one): outbox requests wait. */
const busy = computed(() => !terminalChecked.value || terminalRunning.value || sending.value || streaming.value);
/** The daemon refuses a turn with its JSON error envelope as the body. */
const errorText = computed(() => {
  const e = chat.value?.error;
  return e ? mapErrorResponse(0, e.message, e.message).message : null;
});
const chats = computed(() => store.chats[props.projectId] ?? []);
const activeId = computed(() => store.activeChatId(props.projectId));

function createChat(id: string, initial: UIMessage[]) {
  return new Chat<UIMessage>({
    id,
    messages: initial,
    transport: new DefaultChatTransport({
      api: daemon.api.chatUrl(props.projectId),
      headers: () => daemon.api.authHeaders(),
      // The daemon keeps the transcript and only reads the new user message; the whole
      // conversation would eventually exceed its request body limit.
      prepareSendMessagesRequest: ({ id, messages }) => ({
        body: { id, messages: messages.slice(-1), provider: provider.value },
      }),
    }),
  });
}

let opening = 0;
async function open(id: string) {
  const seq = ++opening;
  void chat.value?.stop();
  chat.value = null;
  chosen.value = null;
  let history: Awaited<ReturnType<typeof daemon.api.chatHistory>> | null = null;
  try {
    history = await daemon.api.chatHistory(props.projectId, id);
  } catch {
    // Not saved yet (a chat with no turns), or a transient failure: start empty.
  }
  if (seq !== opening) return;
  if (history?.provider) chosen.value = history.provider;
  chat.value = createChat(id, history?.messages ?? []);
  if (history?.running) void chat.value.resumeStream();
}

/** Catch up with the saved transcript (a Stop's checkpoint, a terminal turn's marker) unless a turn is streaming here. */
async function reload(id: string) {
  const current = chat.value;
  const idle = () => chat.value === current && !streaming.value;
  if (id !== activeId.value || !current || !idle()) return;
  try {
    const history = await daemon.api.chatHistory(props.projectId, id);
    if (idle()) current.messages = history.messages;
  } catch {
    // Keep what we have; the next turn or reopen catches up.
  }
}

watch(
  activeId,
  (id) => {
    if (id) opened = open(id);
    else startNew();
  },
  { immediate: true },
);

function startNew() {
  historyOpen.value = false;
  store.newChat(props.projectId);
}

async function refreshTerminal() {
  try {
    terminalRunning.value = (await daemon.api.terminalStatus(props.projectId)).running;
  } catch {
    // Keep the last known state.
  } finally {
    terminalChecked.value = true;
  }
}

watch(
  () => props.active,
  (active) => {
    if (!active) return;
    void refreshTerminal();
    void store.load(props.projectId);
    if (!terminal.providers.length) void terminal.loadProviders();
  },
  { immediate: true },
);

const offs = [
  daemon.on("terminal.exited", (e) => {
    if (e.projectId === props.projectId) terminalRunning.value = false;
  }),
  daemon.on("terminal.session", (e) => {
    if (e.projectId !== props.projectId) return;
    void refreshTerminal();
    void reload(e.sessionId);
  }),
  daemon.on("chat.turn", (e) => {
    if (e.projectId === props.projectId) void reload(e.chatId);
  }),
];
onBeforeUnmount(() => {
  offs.forEach((off) => off());
  void chat.value?.stop();
});

async function send(text: string) {
  const trimmed = text.trim();
  if (!trimmed) return;
  sending.value = true;
  try {
    let pending: Promise<void>;
    do {
      pending = opened;
      await pending;
    } while (pending !== opened);
    await chat.value?.sendMessage({ text: trimmed });
  } finally {
    sending.value = false;
  }
}

function submit(text: string) {
  input.value = "";
  const full = expandReferences(text, references);
  references.clear();
  void send(full);
}

async function stop() {
  void chat.value?.stop();
  try {
    await daemon.api.stopChat(props.projectId);
  } catch (e) {
    toast.error(`Couldn't stop the agent: ${errorMessage(e)}`);
  }
}

async function stopTerminal() {
  try {
    await daemon.api.stopTerminal(props.projectId);
  } catch (e) {
    toast.error(`Couldn't stop the terminal session: ${errorMessage(e)}`);
  }
  await refreshTerminal();
}

function insert(text: string) {
  input.value = input.value ? `${input.value} ${text}` : text;
  composer.value?.focus();
}

/** Show a picked element as a short tag; `submit` puts the full reference back. */
function insertReference(text: string) {
  const tag = referenceTag(references.size + 1);
  references.set(tag, text.trim());
  insert(tag);
}

function suggest(text: string) {
  input.value = text;
  composer.value?.focus();
}

defineExpose({ insert, insertReference, send, busy });
</script>

<template>
  <div data-testid="chat-panel" class="bg-background text-foreground flex h-full min-h-0 flex-col">
    <div class="flex h-11 shrink-0 items-center gap-1 border-b px-1.5">
      <ChatHistory
        v-model:open="historyOpen"
        :chats="chats"
        :active-id="activeId"
        :running-id="store.running[projectId] ?? null"
        @pick="store.setActive(projectId, $event)"
      />
      <span class="min-w-0 flex-1 truncate px-1 text-sm">
        {{ chats.find((c) => c.id === activeId)?.title || "New chat" }}
      </span>
      <Button variant="ghost" size="icon-sm" aria-label="New chat" data-testid="chat-new" @click="startNew">
        <Plus />
      </Button>
    </div>

    <Empty v-if="messages.length === 0" class="min-h-0">
      <EmptyHeader>
        <EmptyMedia variant="icon"><MessageSquare /></EmptyMedia>
        <EmptyTitle>What should we change?</EmptyTitle>
        <EmptyDescription>The agent edits your site and saves a checkpoint after each turn.</EmptyDescription>
      </EmptyHeader>
      <EmptyContent class="flex flex-col gap-2">
        <Button
          v-for="s in SUGGESTIONS"
          :key="s"
          variant="outline"
          size="sm"
          class="h-auto w-full justify-start py-2 text-left whitespace-normal"
          data-testid="chat-suggestion"
          @click="suggest(s)"
        >
          {{ s }}
        </Button>
      </EmptyContent>
    </Empty>
    <MessageScrollerProvider v-else>
      <MessageScroller class="min-h-0 flex-1">
        <MessageScrollerViewport>
          <MessageScrollerContent class="flex flex-col gap-4 p-3">
            <MessageScrollerItem v-for="m in messages" :key="m.id" :message-id="m.id">
              <ChatMessage :project-id="projectId" :message="m" @open-terminal="$emit('open-terminal')" />
            </MessageScrollerItem>
          </MessageScrollerContent>
        </MessageScrollerViewport>
        <MessageScrollerButton class="absolute bottom-2 left-1/2 -translate-x-1/2" />
      </MessageScroller>
    </MessageScrollerProvider>

    <div class="flex shrink-0 flex-col gap-2 p-2">
      <div
        v-if="streaming"
        class="bg-background/95 text-muted-foreground flex items-center gap-1.5 self-center rounded-full border px-3 py-1 text-xs shadow-sm"
        role="status"
        data-testid="chat-working"
      >
        <LoaderCircle class="size-3.5 animate-spin" aria-hidden="true" />
        Agent is working…
      </div>
      <Alert v-if="errorText" variant="destructive">
        <CircleAlert />
        <AlertDescription data-testid="chat-error">{{ errorText }}</AlertDescription>
      </Alert>
      <div v-if="terminalRunning" class="text-muted-foreground flex items-center justify-between gap-2 text-sm">
        <span>The agent is busy in Terminal</span>
        <Button variant="outline" size="sm" data-testid="chat-stop-terminal" @click="stopTerminal">
          Stop terminal session
        </Button>
      </div>
      <ChatComposer
        ref="composer"
        v-model="input"
        v-model:provider="provider"
        :providers="providers"
        :provider-locked="messages.length > 0"
        :streaming="streaming"
        :blocked="terminalRunning ? 'The agent is busy in Terminal' : chat ? null : 'Loading chat'"
        @submit="submit"
        @stop="stop"
      />
    </div>
  </div>
</template>
