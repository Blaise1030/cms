<script setup lang="ts">
import { computed, onBeforeUnmount, ref, shallowRef, watch } from "vue";
import { Chat } from "@ai-sdk/vue";
import { DefaultChatTransport, type UIMessage } from "ai";
import { CircleAlert, MessageSquare, Plus } from "@lucide/vue";
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
import { Spinner } from "@/components/ui/spinner";
import { useAgentProvidersStore } from "@/modules/chat/providers";
import ChatComposer, { type ChatAttachment } from "./ChatComposer.vue";
import ChatHistory from "./ChatHistory.vue";
import ChatMessage from "./ChatMessage.vue";
import { attachmentMessage } from "./attachments";
import { expandReferences, referenceTag } from "./references";
import { useChatStore } from "./store";

const props = defineProps<{ projectId: string; active: boolean }>();

/** Terminal providers the daemon can also drive headlessly. */
const CHAT_PROVIDERS = ["cursor", "claude", "codex", "opencode"];
const SUGGESTIONS = [
  "Write a new blog post about our latest update",
  "Make the homepage headline punchier",
  "Fix typos on the About page",
];

const daemon = useDaemon();
const store = useChatStore();
const agentProviders = useAgentProvidersStore();

const chat = shallowRef<Chat<UIMessage> | null>(null);
const input = ref("");
const composer = ref<InstanceType<typeof ChatComposer> | null>(null);
const historyOpen = ref(false);
const terminalRunning = ref(false);
/** Until the first terminal status arrives, `terminalRunning` may be stale. */
const terminalChecked = ref(false);
/** A `send` is waiting for the history or for its turn to end. */
const sending = ref(false);
/** The saved transcript is being fetched; the message list is empty until it lands. */
const loadingChat = ref(false);
/** The chat's own provider, or the user's pick; null falls back to the saved pick. */
const chosen = ref<string | null>(null);
/** The latest `open()`; sends wait for it so none land while the history loads. */
let opened: Promise<void> = Promise.resolve();
/** What each `[Element #n]` in the input stands for, until the input is sent. */
const references = new Map<string, string>();
/** Files uploaded for the next message; the agent reads them from their saved paths. */
const attachments = ref<ChatAttachment[]>([]);
const attaching = ref(false);

async function attach(files: File[]) {
  if (!files.length) return;
  attaching.value = true;
  try {
    for (const file of files) {
      const path = await daemon.api.attachTerminalImage(props.projectId, file);
      attachments.value.push({ name: file.name, path });
    }
  } catch (e) {
    toast.error(`Couldn't attach file: ${errorMessage(e)}`);
  } finally {
    attaching.value = false;
  }
}

const providers = computed(() => agentProviders.providers.map((p) => p.id).filter((id) => CHAT_PROVIDERS.includes(id)));
const provider = computed({
  get: () =>
    chosen.value ??
    (agentProviders.provider && providers.value.includes(agentProviders.provider)
      ? agentProviders.provider
      : providers.value[0]) ??
    null,
  set: (id) => (chosen.value = id),
});
const models = computed(() => (provider.value ? (agentProviders.models[provider.value] ?? []) : []));
/** The saved pick while the agent still offers it; `null` is the agent's default. */
const model = computed({
  get: () => {
    const pick = provider.value ? agentProviders.modelPick(provider.value) : null;
    return models.value.some((m) => m.id === pick) ? pick : null;
  },
  set: (id) => {
    if (provider.value) agentProviders.setModel(provider.value, id);
  },
});
watch(providers, (ids) => ids.forEach((id) => void agentProviders.loadModels(id)), { immediate: true });
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
        body: { id, messages: messages.slice(-1), provider: provider.value, ...(model.value && { model: model.value }) },
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
  loadingChat.value = true;
  let history: Awaited<ReturnType<typeof daemon.api.chatHistory>> | null = null;
  try {
    history = await daemon.api.chatHistory(props.projectId, id);
  } catch {
    // Not saved yet (a chat with no turns), or a transient failure: start empty.
  }
  if (seq !== opening) return;
  loadingChat.value = false;
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

async function removeChat(id: string) {
  try {
    await store.remove(props.projectId, id);
  } catch (e) {
    toast.error(`Couldn't delete the chat: ${errorMessage(e)}`);
  }
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
    if (!agentProviders.providers.length) void agentProviders.loadProviders();
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
  const expanded = expandReferences(text, references);
  references.clear();
  const paths = attachments.value.map((a) => a.path);
  attachments.value = [];
  void send(attachmentMessage(expanded, paths));
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
    <div data-testid="chat-header" class="flex h-11 shrink-0 items-center gap-1 px-1.5">
      <ChatHistory
        v-model:open="historyOpen"
        :chats="chats"
        :active-id="activeId"
        :running-id="store.running[projectId] ?? null"
        @pick="store.setActive(projectId, $event)"
        @remove="removeChat"
      />
      <span class="min-w-0 flex-1 truncate px-1 text-sm">
        {{ chats.find((c) => c.id === activeId)?.title || "New chat" }}
      </span>
      <Button variant="ghost" size="icon-sm" aria-label="New chat" data-testid="chat-new" @click="startNew">
        <Plus />
      </Button>
    </div>

    <div
      v-if="loadingChat"
      class="text-muted-foreground flex min-h-0 flex-1 items-center justify-center gap-2 text-sm"
      role="status"
      data-testid="chat-loading"
    >
      <Spinner class="size-4" aria-hidden="true" />
      <span>Loading chat…</span>
    </div>
    <Empty v-else-if="messages.length === 0" class="min-h-0 px-3 md:px-3">
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
    <MessageScrollerProvider v-else auto-scroll>
      <MessageScroller class="min-h-0 flex-1">
        <MessageScrollerViewport class="scroll-fade-y" data-testid="chat-viewport">
          <!-- The working indicator floats over the bottom edge; keep the last line clear of it. -->
          <MessageScrollerContent
            data-testid="chat-messages"
            :class="['flex flex-col gap-4 p-3', streaming && 'pb-10']"
          >
            <MessageScrollerItem v-for="m in messages" :key="m.id" :message-id="m.id">
              <ChatMessage :project-id="projectId" :message="m" />
            </MessageScrollerItem>
          </MessageScrollerContent>
        </MessageScrollerViewport>
        <MessageScrollerButton class="absolute bottom-2 left-1/2 -translate-x-1/2" />
      </MessageScroller>
    </MessageScrollerProvider>

    <div class="relative flex shrink-0 flex-col gap-2 p-2">
      <Transition
        enter-active-class="transition duration-200 ease-out"
        enter-from-class="translate-y-2 opacity-0"
        leave-active-class="transition duration-150 ease-in"
        leave-to-class="translate-y-2 opacity-0"
      >
        <div
          v-if="streaming"
          class="pointer-events-none absolute bottom-full left-3 flex items-center gap-1.5 py-1 text-xs text-muted-foreground"
          role="status"
          data-testid="chat-working"
        >
          <Spinner class="size-4" aria-hidden="true" />
          <span class="shimmer motion-reduce:shimmer-none">Agent is working…</span>
        </div>
      </Transition>
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
        v-model:model="model"
        :providers="providers"
        :models="agentProviders.models"
        :provider-locked="messages.length > 0"
        :streaming="streaming"
        :blocked="terminalRunning ? 'The agent is busy in Terminal' : chat ? null : 'Loading chat'"
        :attachments="attachments"
        :attaching="attaching"
        @submit="submit"
        @stop="stop"
        @attach="attach"
        @remove-attachment="attachments.splice($event, 1)"
      />
    </div>
  </div>
</template>
