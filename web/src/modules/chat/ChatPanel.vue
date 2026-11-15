<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useRoute } from "vue-router";
import type { UIMessage } from "ai";
import { toast } from "@/lib/toast";
import { errorMessage } from "@orso/utils";
import { ApiError, mapErrorResponse } from "@/core/api";
import { useDaemon } from "@/core/daemon";
import type { ChatHistory as ChatHistoryData, UiChunk } from "@/core/types";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UBadge from "@nuxt/ui/components/Badge.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UCard from "@nuxt/ui/components/Card.vue";
import UEmpty from "@nuxt/ui/components/Empty.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import UChatShimmer from "@nuxt/ui/components/ChatShimmer.vue";
import { useProjectsStore } from "@/modules/projects/store";
import { useAssistantStore } from "@/modules/assistant/store";
import { openAssistantSetup } from "@/modules/assistant/dialog";
import NewChangeDialog from "@/modules/changes/NewChangeDialog.vue";
import { useAgentProvidersStore } from "@/modules/chat/providers";
import { useChangesStore } from "@/modules/changes/store";
import { usePreviewStore } from "@/modules/preview/store";
import { projectTabFromRoute, type ProjectTab } from "@/router";
import ChatComposer, { type ChatAttachment, type ChatReference } from "./ChatComposer.vue";
import ChatHistory from "./ChatHistory.vue";
import ChatMessage from "./ChatMessage.vue";
import ChatTranscript from "./ChatTranscript.vue";
import { attachmentMessage } from "./attachments";
import { createTranscript, type Transcript } from "./chunks";
import {
  clearStickyPageContext,
  nextStickyPageContext,
  resolvePageContext,
  splitPageContext,
  withPageContext,
  type StickyPageContext,
} from "./pageContext";
import { PHASE_LABEL, SYNC_ACTIONS, syncIsTerminal, syncPhaseBadge } from "./parts";
import { referenceLabel } from "./references";
import { spawnRecovery, type RecoveryAction } from "./recovery";
import { buildChatStreamUrl, createChatStream, type ChatStream } from "./stream";
import { useChatStore } from "./store";

const props = withDefaults(
  defineProps<{
    projectId: string;
    active: boolean;
    /** Floating layout adds minimize/close beside the usual header actions. */
    layout?: "sidebar" | "floating";
  }>(),
  { layout: "sidebar" },
);
const emit = defineEmits<{ minimize: []; close: [] }>();
const route = useRoute();
const preview = usePreviewStore();

/** Assistants this UI can drive. */
const CHAT_PROVIDERS = ["cursor", "claude", "codex", "opencode"];
const SUGGESTIONS = [
  "Write a new blog post about our latest update",
  "Make the homepage headline punchier",
  "Fix typos on the About page",
];

const daemon = useDaemon();
const store = useChatStore();
const changes = useChangesStore();
const agentProviders = useAgentProvidersStore();
const assistant = useAssistantStore();
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

/** What kind of tool the agent wants to run, in words. */
const PERMISSION_KIND: Record<string, string> = {
  execute: "Run a command",
  edit: "Edit a file",
  delete: "Delete a file",
  move: "Move a file",
  read: "Read a file",
  search: "Search",
  fetch: "Fetch a web page",
};

function permissionKind(chunk: PermissionChunk): string {
  const kind = chunk.tool_call.kind;
  return (typeof kind === "string" && PERMISSION_KIND[kind.toLowerCase()]) || "Run a tool";
}

/** The exact command (or the tool's title) the agent is asking to run. */
function permissionCommand(chunk: PermissionChunk): string {
  const raw = chunk.tool_call.rawInput as { command?: unknown } | null | undefined;
  if (typeof raw?.command === "string" && raw.command.trim()) return raw.command;
  if (Array.isArray(raw?.command) && raw.command.length) return raw.command.join(" ");
  const title = chunk.tool_call.title;
  return typeof title === "string" && title ? title : "Run a tool";
}

/** Commands longer than this many lines start clipped, with "Show all". */
const PERMISSION_PREVIEW_LINES = 10;
const expandedPermissions = ref(new Set<string>());
const permissionIsLong = (chunk: PermissionChunk) =>
  permissionCommand(chunk).split("\n").length > PERMISSION_PREVIEW_LINES;
function togglePermission(chunk: PermissionChunk) {
  const next = new Set(expandedPermissions.value);
  if (next.has(chunk.request_id)) next.delete(chunk.request_id);
  else next.add(chunk.request_id);
  expandedPermissions.value = next;
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
/** Prompt held while the writer names a new change after `CHANGE_REQUIRED`. */
const pendingChangePrompt = ref<string | null>(null);
const showStartChange = ref(false);
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
/** Elements picked in the preview, sent ahead of the typed text. */
const references = ref<ChatReference[]>([]);
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

const providers = computed(() => {
  // Once the assistant snapshot is known it is the only source of truth: a
  // provider is offered only when chat can start with it now.
  const ids = (assistant.snapshot
    ? assistant.chatReadyProviders.map((p) => p.id)
    : agentProviders.providers.map((p) => p.id)
  ).filter((id) => CHAT_PROVIDERS.includes(id));
  // An open chat keeps its assistant so its transcript, model and send target
  // still resolve even if it needs a sign-in or an update right now.
  if (chatProvider.value && !ids.includes(chatProvider.value)) ids.push(chatProvider.value);
  return ids;
});
const provider = computed({
  get: () => {
    if (chatProvider.value) return chatProvider.value;
    const pick = chosen.value ?? agentProviders.provider;
    if (pick && providers.value.includes(pick)) return pick;
    return providers.value[0] ?? null;
  },
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
/** The chat summary behind the open transcript: its kind and, for a sync, its phase. */
const activeSummary = computed(() => chats.value.find((c) => c.id === activeId.value) ?? null);
const isSync = computed(() => activeSummary.value?.kind === "sync");
const syncPhase = computed(() => activeSummary.value?.sync?.phase ?? null);
const syncTerminal = computed(() => syncIsTerminal(syncPhase.value));
/** A successful combine abandoned its conflict session, so neither terminal
 * action can work: the chat is read-only and done. They remain for `ended` and
 * `expired`, where the session still exists. */
const syncFinalActions = computed(
  () => isSync.value && (syncPhase.value === "ended" || syncPhase.value === "expired"),
);
/** A finished sync chat is final: its transcript must not be followed or reconnected. */
const syncFinished = computed(() => isSync.value && syncTerminal.value);
/** The combine this chat is doing; the chat summary is the source of truth for every action. */
const activeSyncSessionId = computed(() => activeSummary.value?.sync?.conflictSessionId ?? undefined);
/** What the composer shows in place of the input while a sync checks the site. */
const composerLocked = computed(() =>
  isSync.value && syncPhase.value === "checking" ? PHASE_LABEL.checking : null,
);
const composerBlocked = computed(() => {
  if (loadingChat.value) return "Loading chat";
  if (composerLocked.value) return composerLocked.value;
  return provider.value ? null : "Set up your assistant to start chatting";
});

/** The action a failed pre-stream send can offer, matching the transcript's cards. */
const errorRecovery = computed<RecoveryAction | null>(() => spawnRecovery(errorText.value));
/** The last prompt actually sent, so a failed turn can be tried again. */
const lastPrompt = ref<string | null>(null);

/** Chunks already rendered; the SSE replays the transcript, so its first frames are duplicates. */
let renderedChunks = 0;
let transcript: Transcript | null = null;
/** Every chunk in the open transcript, so it can be rebuilt once the chat's kind is known. */
let chunkLog: UiChunk[] = [];
let stream: ChatStream | null = null;
let streamChatId: string | null = null;

/** Rebuild the open transcript from its chunks, hiding agent steps for a sync chat. */
function buildTranscript() {
  transcript = createTranscript({ hideSteps: isSync.value });
  for (const chunk of chunkLog) transcript.push(chunk);
  renderedChunks = chunkLog.length;
  publishTranscript();
}

/** Rebuild the transcript from a snapshot; streamed chunks then append to it. */
function setTranscript(chunks: UiChunk[]) {
  chunkLog = chunks.slice();
  buildTranscript();
}

// The chat list can land after the transcript, so the summary's kind may arrive late.
watch(isSync, (sync, was) => {
  if (sync && !was && transcript) buildTranscript();
});

/**
 * Hand Vue a new list with a new identity for the message that changed, so ChatMessage
 * re-renders even though the builder mutates its last message in place.
 */
function publishTranscript() {
  if (!transcript) return;
  const next = transcript.messages.slice();
  const last = next.length - 1;
  if (last >= 0) next[last] = { ...next[last]! };
  messages.value = next;
}

function stopStream() {
  stream?.close();
  stream = null;
  streamChatId = null;
}

/** Follow `id` live. `restart` reconnects so a fresh snapshot is followed from its end. */
function startStream(id: string, restart = false) {
  // A terminal sync has nothing left to stream; reconnecting would keep an SSE
  // response open until the daemon shuts down.
  if (syncFinished.value) {
    stopStream();
    return;
  }
  if (!restart && stream && streamChatId === id) return;
  stopStream();
  const next = createChatStream({
    url: () => buildChatStreamUrl(daemon.config.value, props.projectId, id),
    skipOnFirstOpen: () => renderedChunks,
    onReset: () => {
      // EventSource reconnected: the daemon replays the transcript from the first chunk.
      // Keep the last snapshot on screen until the replay's first frame replaces it.
      chunkLog = [];
      transcript = createTranscript({ hideSteps: isSync.value });
      renderedChunks = 0;
      firstVisibleMessage.value = 0;
    },
    onChunk: (chunk) => applyChunk(id, chunk),
  });
  if (!next.connect()) return;
  stream = next;
  streamChatId = id;
}

function applyChunk(id: string, chunk: UiChunk) {
  if (id !== activeId.value) return;
  transcript ??= createTranscript({ hideSteps: isSync.value });
  chunkLog.push(chunk);
  transcript.push(chunk);
  publishTranscript();
  if (chunk.type === "turn_end") {
    // The turn is over even if its `chat.turn` event never arrives.
    store.clearChatSending(props.projectId, id);
    void drainQueue();
  } else if (
    chunk.type === "permission"
    && !permissions.value.some((p) => p.request_id === chunk.request_id)
  ) {
    permissions.value.push(chunk);
  }
}

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
  setTranscript(history.chunks);
  permissions.value = pendingPermissions(history);
  firstVisibleMessage.value = Math.max(0, messages.value.length - 10);
  store.setChatRunning(props.projectId, id, history.running);
  if (id === activeId.value) startStream(id);
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
    setTranscript(history.chunks);
    permissions.value = pendingPermissions(history);
    store.setChatRunning(props.projectId, id, history.running);
    // Reconnect so the stream skips this snapshot and then follows live chunks,
    // including the checkpoint the daemon records just after the turn.
    startStream(id, true);
  } catch {
    // Keep what we have; the next turn or reopen catches up.
  }
}

watch(
  historyOpen,
  (open) => {
    // Sessions are created by other sends/tabs too; the popover is the moment to be current.
    if (open) void store.load(props.projectId);
  },
);

watch(
  activeId,
  (id) => {
    stopStream();
    if (id) void open(id);
    else startNew();
  },
  { immediate: true },
);

/** The sync just ended (or landed): stop following its now-final transcript. */
watch(syncFinished, (done) => {
  if (done) stopStream();
});

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
    if (!active) {
      stopStream();
      return;
    }
    void store.load(props.projectId).then(() => {
      // The chat list may arrive after the history loaded; follow it once we know it exists.
      const id = activeId.value;
      if (id) void open(id).then((history) => {
        if (history && id === activeId.value) startStream(id);
      });
    });
    if (!agentProviders.providers.length) void agentProviders.loadProviders();
    if (!assistant.snapshot) void assistant.load();
  },
  { immediate: true },
);

/**
 * A download or sign-in just finished: re-read readiness so the picker gains
 * the assistant without a reload. Progress events do not trigger this.
 */
watch(
  () => assistant.providers
    .map((p) => `${p.id}:${p.runtime?.phase ?? (p.available ? "ready" : "notDownloaded")}`)
    .join("|"),
  (now, before) => {
    if (!before || now === before) return;
    const previous = new Map(before.split("|").map((entry) => entry.split(":") as [string, string]));
    const finished = now.split("|").some((entry) => {
      const [id, phase] = entry.split(":") as [string, string];
      return phase === "ready" && previous.has(id) && previous.get(id) !== "ready";
    });
    if (finished) void assistant.load();
  },
);
watch(
  () => Object.values(assistant.signIns).map((state) => state?.state ?? "").join("|"),
  (now, before) => {
    if (before && now !== before && now.includes("connected")) void assistant.load();
  },
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
onBeforeUnmount(() => {
  offs.forEach((off) => off());
  stopStream();
});

async function sendNow(text: string): Promise<boolean> {
  const trimmed = text.trim();
  if (!trimmed) return false;
  let id = activeId.value ?? store.newChat(props.projectId);
  lastPrompt.value = trimmed;
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
    if (!assistant.snapshot && !agentProviders.providers.length) await agentProviders.loadProviders();
    const providerId = requestedProvider ?? providers.value[0] ?? null;
    if (!providerId) throw new Error("Set up your assistant to start chatting");
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
    // A chat is created by its first message; refresh the list so the new session
    // shows (with its title and running mark) before the turn ends.
    void store.load(props.projectId);
    return true;
  } catch (e) {
    if (optimisticId && id === activeId.value) messages.value = messages.value.filter(m => m.id !== optimisticId);
    store.clearChatSending(props.projectId, id);
    if (id === activeId.value) {
      if (e instanceof ApiError && e.code === "CHANGE_REQUIRED") {
        errorText.value = null;
        pendingChangePrompt.value = trimmed;
        showStartChange.value = true;
      } else {
        const message = errorMessage(e);
        errorText.value = mapErrorResponse(0, message, message).message;
      }
    }
    if (id && store.queued[props.projectId]?.[id]?.length) store.setQueuePaused(props.projectId, id, true);
    return false;
  } finally {
    sendingChats.value[id] = false;
  }
}

watch(showStartChange, (open) => {
  if (open || !pendingChangePrompt.value) return;
  // Cancelled: put the prompt back so the writer can edit or try again.
  if (!input.value) input.value = splitPageContext(pendingChangePrompt.value).text;
  pendingChangePrompt.value = null;
});

async function onChangeCreated() {
  const text = pendingChangePrompt.value;
  pendingChangePrompt.value = null;
  showStartChange.value = false;
  if (text) await sendNow(text);
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

/** Sticky page context: live on Preview/Content, frozen after leaving those modules. */
const stickyPageContext = ref<StickyPageContext>({ context: null, frozen: false });
const pageContext = computed(() => stickyPageContext.value.context);

function currentPageTab(): ProjectTab {
  return projectTabFromRoute(route?.name);
}

function resolvedPageContext() {
  const tab = currentPageTab();
  return resolvePageContext({
    tab,
    params: {
      path: preview.framePathFor(props.projectId),
      collection: typeof route?.params?.collection === "string" ? route.params.collection : undefined,
      entry: typeof route?.params?.entry === "string" ? route.params.entry : undefined,
    },
  });
}

function syncStickyPageContext() {
  stickyPageContext.value = nextStickyPageContext(
    stickyPageContext.value,
    currentPageTab(),
    resolvedPageContext(),
  );
}

watch(
  () => [
    route?.name,
    route?.params?.collection,
    route?.params?.entry,
    preview.framePathFor(props.projectId),
    props.projectId,
  ],
  syncStickyPageContext,
  { immediate: true },
);

function clearPageContext() {
  stickyPageContext.value = clearStickyPageContext();
}

async function send(text: string, previewText = text) {
  const trimmed = text.trim();
  if (!trimmed) return;
  const id = activeId.value ?? store.newChat(props.projectId);
  if (streaming.value || store.isQueuePaused(props.projectId, id) || store.queued[props.projectId]?.[id]?.length) {
    store.enqueue(props.projectId, id, trimmed, previewText);
    if (!streaming.value) void drainQueue();
    return;
  }
  await sendNow(trimmed);
}

/** Outbox / external sends: inject the current page context; keep the queue preview clean. */
function sendWithPageContext(text: string, previewText = text) {
  void send(withPageContext(text, pageContext.value), previewText);
}

function submit(text: string) {
  const picked = references.value.map((r) => r.full);
  const typed = text.trim();
  const previewText = typed
    || references.value.map((r) => r.label).join(", ")
    || attachments.value.map((a) => a.name).join(", ");
  input.value = "";
  references.value = [];
  const paths = attachments.value.map((a) => a.path);
  attachments.value = [];
  // Context after the typed text; attachment paths stay last so display splitting still works.
  void send(
    attachmentMessage(
      withPageContext([...picked, typed].filter(Boolean).join(" "), pageContext.value),
      paths,
    ),
    previewText,
  );
}

async function stop() {
  const id = activeId.value;
  if (!id) return;
  store.setQueuePaused(props.projectId, id, true);
  try {
    await daemon.api.stopChat(props.projectId, id);
  } catch (e) {
    toast.error(`Couldn't stop your assistant: ${errorMessage(e)}`);
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

/** Open the one assistant setup dialog; sign-in and download live there. */
function openSetup() {
  openAssistantSetup();
}

/** The last thing the user asked, from this session or the rendered transcript. */
function lastUserText(): string | null {
  for (let i = messages.value.length - 1; i >= 0; i -= 1) {
    const message = messages.value[i]!;
    if (message.role !== "user") continue;
    const text = message.parts
      .flatMap((part) => (part.type === "text" ? [part.text] : []))
      .join("\n")
      .trim();
    if (text) return text;
  }
  return null;
}

function retryLast() {
  errorText.value = null;
  const id = activeId.value;
  if (id) store.setQueuePaused(props.projectId, id, false);
  const text = lastPrompt.value ?? lastUserText();
  if (!text || busy.value) {
    composer.value?.focus();
    return;
  }
  void send(text);
}

/** Start the provider's sign-in and put its steps in front of the user. */
function startSignIn(id?: string | null) {
  const target =
    id || provider.value || assistant.chatReadyProviders[0]?.id || assistant.recommended?.id || "";
  if (target) void assistant.signIn(target);
  openAssistantSetup();
}

function handleAssistantAction(action: "setup" | "signIn" | "retry", providerId: string) {
  if (action === "setup") openAssistantSetup();
  else if (action === "signIn") startSignIn(providerId);
  else retryLast();
}

/** A send failed before the turn started: offer the same recovery as a card. */
function recoverError() {
  if (errorRecovery.value === "signIn") startSignIn(provider.value);
  else if (errorRecovery.value === "setup") openAssistantSetup();
}

/** Carry a picked element into the next message as a badge. */
function insertReference(text: string) {
  const full = text.trim();
  if (full && !references.value.some((r) => r.full === full)) {
    references.value.push({ label: referenceLabel(full), full });
  }
  composer.value?.focus();
}

function suggest(text: string) {
  input.value = text;
  composer.value?.focus();
}

function loadEarlierMessages() {
  firstVisibleMessage.value = Math.max(0, firstVisibleMessage.value - 10);
}

defineExpose({ insert, insertReference, send: sendWithPageContext, busy });
</script>

<template>
  <div data-testid="chat-panel" class="bg-background text-default flex h-full min-h-0 flex-col">
    <div data-testid="chat-header" class="flex h-11 shrink-0 items-center gap-1 px-1.5">
      <ChatHistory
        v-if="layout !== 'floating'"
        v-model:open="historyOpen"
        :chats="chats"
        :active-id="activeId"
        :running-ids="store.runningChatIds(projectId)"
        @pick="store.setActive(projectId, $event)"
        @remove="removeChat"
      />
      <span class="min-w-0 flex-1 truncate px-1 text-sm">
        {{ activeSummary?.title || "New chat" }}
      </span>
      <UBadge
        v-if="syncPhase"
        color="neutral"
        :variant="syncPhaseBadge(syncPhase) === 'outline' ? 'outline' : 'soft'"
        data-testid="sync-phase"
        class="shrink-0"
        :label="PHASE_LABEL[syncPhase]"
      />
      <UButton
        v-if="isSync && !syncTerminal"
        color="neutral"
        variant="ghost"
        size="sm"
        data-testid="sync-end"
        :label="SYNC_ACTIONS.end"
        @click="changes.endSyncChat(projectId, activeSyncSessionId, activeId ?? undefined)"
      />
      <UButton
        v-if="layout !== 'floating'"
        color="neutral"
        variant="ghost"
        size="sm"
        icon="i-hugeicons-plus-sign"
        aria-label="New chat"
        data-testid="chat-new"
        @click="startNew"
      />
      <template v-if="layout === 'floating'">
        <UButton
          color="neutral"
          variant="ghost"
          size="sm"
          icon="i-hugeicons-minus-sign"
          aria-label="Minimize agent"
          data-testid="agent-minimize"
          @click="emit('minimize')"
        />
        <UButton
          color="neutral"
          variant="ghost"
          size="sm"
          icon="i-hugeicons-cancel-01"
          aria-label="Close agent"
          data-testid="agent-close"
          @click="emit('close')"
        />
      </template>
    </div>

    <div
      v-if="loadingChat"
      class="text-muted flex min-h-0 flex-1 items-start gap-2 p-3 text-sm"
      role="status"
      data-testid="chat-loading"
    >
      <UIcon name="i-hugeicons-loading-03" class="size-4 animate-spin" aria-hidden="true" />
      <span>Loading chat…</span>
    </div>
    <UEmpty
      v-else-if="messages.length === 0"
      variant="naked"
      icon="i-hugeicons-message-01"
      title="What should we change?"
      description="Your assistant edits your site and saves a checkpoint after each turn."
      size="sm"
      class="min-h-0 flex-1 px-3"
      :ui="{ body: 'w-full', actions: 'w-full' }"
    >
      <template #actions>
        <div class="flex w-full flex-col gap-2">
          <UButton
            v-for="s in SUGGESTIONS"
            :key="s"
            color="neutral"
            variant="outline"
            size="sm"
            block
            class="justify-start py-2 text-left whitespace-normal"
            data-testid="chat-suggestion"
            :label="s"
            @click="suggest(s)"
          />
        </div>
      </template>
    </UEmpty>
    <!-- The working indicator floats over the bottom edge; keep the last line clear of it. -->
    <ChatTranscript
      v-else
      :messages="visibleMessages"
      :streaming="streaming"
      :can-load-earlier="firstVisibleMessage > 0"
      :list-class="['gap-4 p-3', streaming && 'pb-10']"
      @load-earlier="loadEarlierMessages"
    >
      <template #default="{ message: m }">
        <ChatMessage
          data-testid="chat-message"
          :project-id="projectId"
          :message="m"
          :provider="provider"
          @assistant-action="handleAssistantAction($event.action, $event.provider)"
        />
      </template>
    </ChatTranscript>

    <div class="relative flex shrink-0 flex-col gap-2 p-2">
      <Transition
        enter-active-class="transition duration-200 ease-out"
        enter-from-class="translate-y-2 opacity-0"
        leave-active-class="transition duration-150 ease-in"
        leave-to-class="translate-y-2 opacity-0"
      >
        <div
          v-if="streaming"
          class="text-muted pointer-events-none absolute bottom-full left-3 flex items-center gap-1.5 py-1 text-xs"
          role="status"
          data-testid="chat-working"
        >
          <UIcon name="i-hugeicons-loading-03" class="size-4 animate-spin" aria-hidden="true" />
          <UChatShimmer text="Working on it…" />
        </div>
      </Transition>
      <UAlert v-if="errorText" role="alert" color="error" variant="subtle" icon="i-hugeicons-alert-circle">
        <template #description>
          <div class="flex flex-col items-start gap-2">
            <span data-testid="chat-error">{{ errorText }}</span>
            <UButton
              v-if="errorRecovery"
              color="neutral"
              variant="outline"
              size="sm"
              data-testid="chat-error-action"
              :label="errorRecovery === 'signIn' ? 'Sign in' : 'Set up'"
              @click="recoverError"
            />
          </div>
        </template>
      </UAlert>
      <!-- Only the frame and icon are amber; the command reads as code, and the actions stay in view. -->
      <UAlert
        v-for="perm in permissions"
        :key="perm.request_id"
        data-testid="chat-approval"
        role="alert"
        color="warning"
        variant="outline"
        icon="i-hugeicons-shield-01"
        title="Approval needed"
        orientation="vertical"
        :ui="{ title: 'text-highlighted', description: 'text-muted min-w-0', wrapper: 'min-w-0' }"
      >
        <template #description>
          <div class="flex min-w-0 flex-col gap-1.5">
            <span class="text-xs" data-testid="chat-approval-kind">{{ permissionKind(perm) }}</span>
            <pre
              :class="[
                'bg-muted text-default border-default overflow-auto rounded-md border px-2 py-1.5 font-mono text-xs whitespace-pre',
                !expandedPermissions.has(perm.request_id) && 'max-h-48',
              ]"
            ><code data-testid="chat-approval-title">{{ permissionCommand(perm) }}</code></pre>
            <UButton
              v-if="permissionIsLong(perm)"
              color="neutral"
              variant="link"
              size="xs"
              class="self-start p-0"
              :label="expandedPermissions.has(perm.request_id) ? 'Show less' : 'Show all'"
              data-testid="chat-approval-toggle"
              @click="togglePermission(perm)"
            />
          </div>
        </template>
        <template #actions>
          <div class="flex w-full justify-end gap-2">
            <UButton color="neutral" variant="outline" size="sm" label="Deny" :disabled="answering" data-testid="chat-approval-deny" @click="answerPermission(perm, false)" />
            <UButton size="sm" label="Allow" :disabled="answering" data-testid="chat-approval-allow" @click="answerPermission(perm, true)" />
          </div>
        </template>
      </UAlert>
      <UCard
        v-if="queued.length"
        data-testid="chat-queued"
        class="text-xs"
        :ui="{ header: 'flex items-center justify-between gap-2 px-3 py-2 sm:px-3', body: 'px-3 py-2 sm:p-3 sm:px-3' }"
      >
        <template #header>
          <span class="text-highlighted text-xs font-semibold">Queued ({{ queued.length }})</span>
          <UButton
            v-if="queuePaused"
            variant="link"
            size="xs"
            class="p-0"
            label="Resume queue"
            data-testid="chat-queue-resume"
            @click="resumeQueue"
          />
        </template>
        <div class="-m-1 flex max-h-28 flex-col gap-1 overflow-y-auto p-1">
          <div v-for="item in queued" :key="item.id" data-testid="chat-queued-item" class="flex items-center gap-2">
            <span class="min-w-0 flex-1 truncate">{{ item.preview }}</span>
            <UButton
              color="neutral"
              variant="outline"
              size="xs"
              icon="i-hugeicons-arrow-up-01"
              class="shrink-0"
              :aria-label="`Send queued message: ${item.preview}`"
              title="Send next"
              data-testid="chat-queued-send"
              @click="sendQueued(item.id)"
            />
            <UButton
              color="neutral"
              variant="outline"
              size="xs"
              icon="i-hugeicons-cancel-01"
              class="text-muted hover:text-error shrink-0"
              :aria-label="`Remove queued message: ${item.preview}`"
              title="Remove"
              data-testid="chat-queued-remove"
              @click="removeQueued(item.id)"
            />
          </div>
        </div>
      </UCard>
      <div v-if="syncFinalActions" data-testid="sync-final" class="flex flex-col gap-2">
        <UButton
          color="neutral"
          variant="outline"
          size="sm"
          block
          data-testid="sync-pick"
          :label="SYNC_ACTIONS.pick"
          @click="changes.pickMyself(projectId, activeSyncSessionId)"
        />
        <UButton
          color="neutral"
          variant="outline"
          size="sm"
          block
          data-testid="sync-again"
          :label="SYNC_ACTIONS.again"
          @click="changes.combineAgain(projectId, activeSyncSessionId)"
        />
      </div>
      <UAlert
        v-if="!provider && !loadingChat && assistant.snapshot && !isSync"
        data-testid="chat-no-assistant"
        role="status"
        color="neutral"
        variant="outline"
        description="Your assistant isn’t set up yet. Set one up to start chatting."
        orientation="vertical"
      >
        <template #actions>
          <UButton size="sm" label="Set up your assistant" data-testid="chat-setup" @click="openSetup" />
        </template>
      </UAlert>
      <ChatComposer
        v-if="!syncTerminal"
        ref="composer"
        v-model="input"
        v-model:provider="provider"
        v-model:model="model"
        :providers="providers"
        :models="agentProviders.models"
        :provider-locked="chatProvider !== null"
        :streaming="streaming"
        :blocked="composerBlocked"
        :locked="composerLocked"
        :attachments="attachments"
        :attaching="attaching"
        :references="references"
        :page-context="pageContext"
        :auto-approve="autoApprove"
        @submit="submit"
        @stop="stop"
        @attach="attach"
        @remove-attachment="attachments.splice($event, 1)"
        @remove-reference="references.splice($event, 1)"
        @clear-page-context="clearPageContext"
        @set-auto-approve="setAutoApprove"
        @setup="openSetup"
      />
    </div>

    <NewChangeDialog
      v-model:open="showStartChange"
      :project-id="projectId"
      heading="Start a change first"
      description="The live site can't be edited directly. Name this change to continue with your prompt."
      @created="onChangeCreated"
    />
  </div>
</template>
