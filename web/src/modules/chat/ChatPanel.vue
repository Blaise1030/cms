<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import type { UIMessage } from "ai";
import { ArrowUp, CircleAlert, MessageSquare, Plus, ShieldQuestion, X } from "@lucide/vue";
import { toast } from "vue-sonner";
import { errorMessage } from "@orso/utils";
import { mapErrorResponse } from "@/core/api";
import { useDaemon } from "@/core/daemon";
import type { ChatHistory as ChatHistoryData, UiChunk } from "@/core/types";
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
import { Switch } from "@/components/ui/switch";
import { useProjectsStore } from "@/modules/projects/store";
import { useAgentProvidersStore } from "@/modules/chat/providers";
import ChatComposer, { type ChatAttachment } from "./ChatComposer.vue";
import ChatHistory from "./ChatHistory.vue";
import ChatMessage from "./ChatMessage.vue";
import { attachmentMessage } from "./attachments";
import { chunksToMessages } from "./chunks";
import { expandReferences, referenceTag } from "./references";
import { useChatStore } from "./store";

const props = defineProps<{ projectId: string; active: boolean }>();

/** Agent CLIs this UI can drive over ACP. */
const CHAT_PROVIDERS = ["cursor", "claude", "codex", "opencode"];
const SUGGESTIONS = [
  "Write a new blog post about our latest update",
  "Make the homepage headline punchier",
  "Fix typos on the About page",
];

const daemon = useDaemon();
const store = useChatStore();
const agentProviders = useAgentProvidersStore();
const projects = useProjectsStore();

type PermissionChunk = Extract<UiChunk, { type: "permission" }>;
/** Permission prompts the agent is blocked on in the open chat. */
const permissions = ref<PermissionChunk[]>([]);
const answering = ref(false);
const autoApprove = computed(() => projects.projects.find((p) => p.id === props.projectId)?.autoApprove ?? false);

function pendingPermissions(history: ChatHistoryData): PermissionChunk[] {
  const pending = new Set(history.pendingPermissions ?? []);
  return history.chunks.filter((c): c is PermissionChunk => c.type === "permission" && pending.has(c.request_id));
}

function permissionTitle(chunk: PermissionChunk): string {
  const title = chunk.tool_call.title;
  return typeof title === "string" && title ? title : "Run a tool";
}

async function answerPermission(chunk: PermissionChunk, allow: boolean) {
  const id = activeId.value;
  if (!id) return;
  answering.value = true;
  try {
    await daemon.api.approveChat(props.projectId, id, chunk.request_id, allow);
    permissions.value = permissions.value.filter((p) => p.request_id !== chunk.request_id);
  } catch (e) {
    toast.error(`Couldn't answer the request: ${errorMessage(e)}`);
  } finally {
    answering.value = false;
  }
}

async function setAutoApprove(on: boolean) {
  try {
    await projects.setAutoApprove(props.projectId, on);
    if (on) for (const chunk of [...permissions.value]) await answerPermission(chunk, true);
  } catch (e) {
    toast.error(`Couldn't change auto-approve: ${errorMessage(e)}`);
  }
}

const messages = ref<UIMessage[]>([]);
/** Index of the earliest rendered message; older history is available on demand. */
const firstVisibleMessage = ref(0);
const visibleMessages = computed(() => messages.value.slice(firstVisibleMessage.value));
const input = ref("");
const composer = ref<InstanceType<typeof ChatComposer> | null>(null);
const historyOpen = ref(false);
/** Pending POSTs are tracked per chat, so one session never blocks another. */
const sendingChats = ref<Record<string, boolean>>({});
/** The saved transcript is being fetched; the message list is empty until it lands. */
const loadingChat = ref(false);
/** The latest API failure, shown above the composer. */
const errorText = ref<string | null>(null);
/** The provider saved on the open chat; null for a draft with no messages yet. */
const chatProvider = ref<string | null>(null);
/** The model saved on the open chat; null is the agent's default. */
const chatModel = ref<string | null>(null);
/** The user's pick for a chat that does not exist yet. */
const chosen = ref<string | null>(null);
/** The latest `open()`; sends wait for it so none land while the history loads. */
let opening = 0;
let openId: string | null = null;
let openPromise: Promise<ChatHistoryData | null> = Promise.resolve(null);
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
      const path = await daemon.api.attachChatFile(props.projectId, file);
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
    chatProvider.value ??
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

const activeId = computed(() => store.activeChatId(props.projectId));
const streaming = computed(() => !!activeId.value
  && (store.isChatRunning(props.projectId, activeId.value) || !!sendingChats.value[activeId.value]));
/** A new turn would be refused (or race the current one): outbox requests wait. */
const busy = computed(() => !store.activeChatId(props.projectId) || loadingChat.value || streaming.value);
const chats = computed(() => store.chats[props.projectId] ?? []);
const queued = computed(() => activeId.value ? (store.queued[props.projectId]?.[activeId.value] ?? []) : []);
const queuePaused = computed(() => !!activeId.value && store.isQueuePaused(props.projectId, activeId.value));
const drainingQueue = new Set<string>();
const isSaved = (id: string) => chats.value.some((c) => c.id === id);

/** Fetch one chat's transcript. Draft ids (not saved yet) come back empty. */
async function loadChat(id: string): Promise<ChatHistoryData | null> {
  const seq = ++opening;
  loadingChat.value = true;
  messages.value = [];
  firstVisibleMessage.value = 0;
  errorText.value = null;
  chatProvider.value = null;
  chatModel.value = null;
  let history: ChatHistoryData | null = null;
  try {
    history = await daemon.api.chatHistory(props.projectId, id);
  } catch {
    // Not saved yet (a draft), or a transient failure: start empty.
  }
  if (seq !== opening) return history;
  loadingChat.value = false;
  if (!history) return null;
  chatProvider.value = history.provider;
  chatModel.value = history.model;
  // The chat's model becomes this agent's pick, so the picker shows what is running.
  agentProviders.setModel(history.provider, history.model);
  messages.value = chunksToMessages(history.chunks);
  permissions.value = pendingPermissions(history);
  firstVisibleMessage.value = Math.max(0, messages.value.length - 10);
  store.setChatRunning(props.projectId, id, history.running);
  return history;
}

/** Open a chat once; concurrent callers share the fetch. */
function open(id: string): Promise<ChatHistoryData | null> {
  if (id === openId) return openPromise;
  openId = id;
  openPromise = loadChat(id);
  return openPromise;
}

function startNew() {
  historyOpen.value = false;
  chosen.value = null;
  store.newChat(props.projectId);
}

/** Catch up with the saved transcript after a turn. */
async function reload(id: string) {
  if (id !== activeId.value) return;
  try {
    const history = await daemon.api.chatHistory(props.projectId, id);
    if (id !== activeId.value) return;
    messages.value = chunksToMessages(history.chunks);
    permissions.value = pendingPermissions(history);
    store.setChatRunning(props.projectId, id, history.running);
  } catch {
    // Keep what we have; the next turn or reopen catches up.
  }
}

watch(
  activeId,
  (id) => {
    if (id) void open(id);
    else startNew();
  },
  { immediate: true },
);

async function removeChat(id: string) {
  try {
    await store.remove(props.projectId, id);
  } catch (e) {
    toast.error(`Couldn't delete the chat: ${errorMessage(e)}`);
  }
}

watch(
  () => props.active,
  (active) => {
    if (!active) return;
    void store.load(props.projectId);
    if (!agentProviders.providers.length) void agentProviders.loadProviders();
  },
  { immediate: true },
);

const offs = [
  daemon.on("chat.turn", (e) => {
    if (e.projectId === props.projectId && e.chatId === activeId.value)
      void reload(e.chatId).then(() => drainQueue());
  }),
  daemon.on("chat.permission", (e) => {
    if (e.projectId === props.projectId && e.chatId === activeId.value) void reload(e.chatId);
  }),
  daemon.onReconnected(() => {
    if (activeId.value) void reload(activeId.value).then(() => drainQueue());
  }),
];
onBeforeUnmount(() => offs.forEach((off) => off()));

async function sendNow(text: string): Promise<boolean> {
  const trimmed = text.trim();
  if (!trimmed) return false;
  let id = activeId.value ?? store.newChat(props.projectId);
  sendingChats.value[id] = true;
  store.markChatSending(props.projectId, id);
  if (id === activeId.value) errorText.value = null;
  const requestedProvider = provider.value;
  const requestedModel = model.value;
  const existingProvider = chatProvider.value;
  let optimisticId: string | null = null;
  try {
    // Wait for the active chat (or draft) to finish opening: a saved chat must not
    // be mistaken for a draft just because the list is still loading.
    const opened = await open(id);
    if (!requestedProvider) await agentProviders.loadProviders();
    const providerId = requestedProvider ?? agentProviders.providers
      .map(p => p.id).find(p => CHAT_PROVIDERS.includes(p));
    if (!providerId) throw new Error("No agent is available");
    if (!isSaved(id) && !opened?.provider && existingProvider === null) {
      const draftId = id;
      const created = await daemon.api.createChat(props.projectId, providerId, requestedModel);
      store.moveQueued(props.projectId, id, created.id);
      sendingChats.value[draftId] = false;
      store.clearChatSending(props.projectId, draftId);
      id = created.id;
      sendingChats.value[id] = true;
      store.markChatSending(props.projectId, id);
      if (activeId.value === draftId) {
        store.setActive(props.projectId, id);
        await open(id);
      }
    } else if (id === activeId.value && requestedModel && requestedModel !== chatModel.value) {
      await daemon.api.setChatModel(props.projectId, id, requestedModel);
      chatModel.value = requestedModel;
    }
    if (id === activeId.value) {
      optimisticId = `u-${crypto.randomUUID()}`;
      messages.value = [
        ...messages.value,
        { id: optimisticId, role: "user", parts: [{ type: "text", text: trimmed }] },
      ];
    }
    await daemon.api.sendChatMessage(props.projectId, id, trimmed);
    return true;
  } catch (e) {
    if (optimisticId && id === activeId.value) messages.value = messages.value.filter(m => m.id !== optimisticId);
    store.clearChatSending(props.projectId, id);
    if (id === activeId.value) {
      const message = errorMessage(e);
      errorText.value = mapErrorResponse(0, message, message).message;
    }
    if (id && store.queued[props.projectId]?.[id]?.length) store.setQueuePaused(props.projectId, id, true);
    return false;
  } finally {
    sendingChats.value[id] = false;
  }
}

async function drainQueue() {
  const id = activeId.value;
  if (!id || drainingQueue.has(id) || streaming.value || loadingChat.value || store.isQueuePaused(props.projectId, id)) return;
  const next = store.queued[props.projectId]?.[id]?.[0];
  if (!next) return;
  drainingQueue.add(id);
  store.removeQueued(props.projectId, id, next.id);
  try {
    if (!(await sendNow(next.body))) {
      store.restoreQueued(props.projectId, id, next);
      store.setQueuePaused(props.projectId, id, true);
    }
  } finally {
    drainingQueue.delete(id);
    // A very fast turn can finish before its POST returns; keep draining then.
    if (!streaming.value) void drainQueue();
  }
}

watch([streaming, loadingChat, activeId], () => void drainQueue(), { flush: "post" });

async function send(text: string, preview = text) {
  const trimmed = text.trim();
  if (!trimmed) return;
  const id = activeId.value ?? store.newChat(props.projectId);
  if (streaming.value || store.isQueuePaused(props.projectId, id) || store.queued[props.projectId]?.[id]?.length) {
    store.enqueue(props.projectId, id, trimmed, preview);
    if (!streaming.value) void drainQueue();
    return;
  }
  await sendNow(trimmed);
}

function submit(text: string) {
  const preview = text.trim() || attachments.value.map(a => a.name).join(", ");
  input.value = "";
  const expanded = expandReferences(text, references);
  references.clear();
  const paths = attachments.value.map((a) => a.path);
  attachments.value = [];
  void send(attachmentMessage(expanded, paths), preview);
}

async function stop() {
  const id = activeId.value;
  if (!id) return;
  store.setQueuePaused(props.projectId, id, true);
  try {
    await daemon.api.stopChat(props.projectId, id);
  } catch (e) {
    toast.error(`Couldn't stop the agent: ${errorMessage(e)}`);
  }
}

function resumeQueue() {
  const id = activeId.value;
  if (!id) return;
  store.setQueuePaused(props.projectId, id, false);
  errorText.value = null;
  void drainQueue();
}

function removeQueued(id: string) {
  if (activeId.value) store.removeQueued(props.projectId, activeId.value, id);
}

function sendQueued(id: string) {
  if (!activeId.value) return;
  store.prioritizeQueued(props.projectId, activeId.value, id);
  resumeQueue();
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

function loadEarlierMessages() {
  firstVisibleMessage.value = Math.max(0, firstVisibleMessage.value - 10);
}

function onMessageScroll(event: Event) {
  const viewport = event.target as HTMLElement;
  if (viewport.scrollTop <= 80 && firstVisibleMessage.value > 0) loadEarlierMessages();
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
        :running-ids="store.runningChatIds(projectId)"
        @pick="store.setActive(projectId, $event)"
        @remove="removeChat"
      />
      <span class="min-w-0 flex-1 truncate px-1 text-sm">
        {{ chats.find((c) => c.id === activeId)?.title || "New chat" }}
      </span>
      <label class="text-muted-foreground flex shrink-0 items-center gap-1.5 text-xs" title="Run tools without asking">
        <span>Auto-approve</span>
        <Switch
          :model-value="autoApprove"
          aria-label="Auto-approve"
          data-testid="chat-auto-approve"
          @update:model-value="setAutoApprove(!!$event)"
        />
      </label>
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
        <MessageScrollerViewport class="scroll-fade-y" data-testid="chat-viewport" @scroll="onMessageScroll">
          <!-- The working indicator floats over the bottom edge; keep the last line clear of it. -->
          <MessageScrollerContent
            data-testid="chat-messages"
            :class="['flex flex-col gap-4 p-3', streaming && 'pb-10']"
          >
            <MessageScrollerItem v-for="m in visibleMessages" :key="m.id" :message-id="m.id">
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
      <div
        v-for="perm in permissions"
        :key="perm.request_id"
        data-testid="chat-approval"
        class="flex flex-col gap-2 rounded-lg border border-amber-500/40 bg-amber-500/10 p-2 text-sm"
        role="alert"
      >
        <div class="flex items-start gap-2">
          <ShieldQuestion class="mt-0.5 size-4 shrink-0" aria-hidden="true" />
          <div class="min-w-0 flex-1">
            <div class="font-medium">Approval needed</div>
            <div class="text-muted-foreground break-words text-xs" data-testid="chat-approval-title">{{ permissionTitle(perm) }}</div>
          </div>
        </div>
        <div class="flex justify-end gap-2">
          <Button size="sm" variant="outline" :disabled="answering" data-testid="chat-approval-deny" @click="answerPermission(perm, false)">Deny</Button>
          <Button size="sm" :disabled="answering" data-testid="chat-approval-allow" @click="answerPermission(perm, true)">Allow</Button>
        </div>
      </div>
      <div v-if="queued.length" data-testid="chat-queued" class="rounded-lg border bg-muted/30 p-2 text-xs">
        <div class="mb-1 flex items-center justify-between gap-2 font-medium">
          <span>Queued ({{ queued.length }})</span>
          <button v-if="queuePaused" type="button" class="text-primary hover:underline" data-testid="chat-queue-resume" @click="resumeQueue">Resume queue</button>
        </div>
        <div class="flex max-h-28 flex-col gap-1 overflow-y-auto">
          <div v-for="item in queued" :key="item.id" data-testid="chat-queued-item" class="flex items-center gap-2">
            <span class="min-w-0 flex-1 truncate">{{ item.preview }}</span>
            <Button variant="ghost" size="icon-sm" class="size-6 shrink-0" :aria-label="`Send queued message: ${item.preview}`" title="Send next" data-testid="chat-queued-send" @click="sendQueued(item.id)">
              <ArrowUp class="size-3.5" aria-hidden="true" />
            </Button>
            <Button variant="ghost" size="icon-sm" class="size-6 shrink-0 text-muted-foreground hover:text-destructive" :aria-label="`Remove queued message: ${item.preview}`" title="Remove" data-testid="chat-queued-remove" @click="removeQueued(item.id)">
              <X class="size-3.5" aria-hidden="true" />
            </Button>
          </div>
        </div>
      </div>
      <ChatComposer
        ref="composer"
        v-model="input"
        v-model:provider="provider"
        v-model:model="model"
        :providers="providers"
        :models="agentProviders.models"
        :provider-locked="chatProvider !== null"
        :streaming="streaming"
        :blocked="loadingChat ? 'Loading chat' : provider ? null : 'No agent is available'"
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
