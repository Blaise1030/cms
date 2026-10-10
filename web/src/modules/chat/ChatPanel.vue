<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { ArrowUp, ChevronDown, Loader2, Shield, ShieldCheck, Sparkle, Square } from "@lucide/vue";
import type { ChatStatus } from "ai";
import {
  Conversation,
  ConversationContent,
  ConversationEmptyState,
  ConversationScrollButton,
} from "@/components/ai-elements/conversation";
import {
  PromptInput,
  PromptInputBody,
  PromptInputFooter,
  PromptInputSubmit,
  PromptInputTools,
  type PromptInputMessage,
} from "@/components/ai-elements/prompt-input";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import NewChangeDialog from "@/modules/changes/NewChangeDialog.vue";
import ChatMessage from "./ChatMessage.vue";
import ComposerEditor from "./ComposerEditor.vue";
import PermissionPrompt from "./PermissionPrompt.vue";
import { usePreviewStore } from "@/modules/preview/store";
import { useChatStore } from "./store";

const props = defineProps<{ projectId: string; autoApprove?: boolean }>();
const emit = defineEmits<{ toggleAutoApprove: [] }>();

const store = useChatStore();
const preview = usePreviewStore();
const composer = ref<InstanceType<typeof ComposerEditor> | null>(null);
const showChangeGuard = ref(false);
const pendingPrompt = ref<string | null>(null);
const pc = computed(() => store.byProject[props.projectId]);
const busy = computed(() => store.isBusy(props.projectId));
const canStop = computed(() => !!pc.value?.turns.some((t) => t.status === "streaming" && !t.pending));
const provider = computed(() => pc.value?.provider ?? store.providers[0]?.id ?? "");

const submitStatus = computed<ChatStatus | undefined>(() => {
  if (pc.value?.turns.some((t) => t.status === "streaming")) return "streaming";
  if (pc.value?.sending) return "submitted";
  if (pc.value?.error) return "error";
  return "ready";
});

watch(
  () => props.projectId,
  (id) => {
    void store.openProject(id);
  },
  { immediate: true },
);

onMounted(() => {
  void store.openProject(props.projectId);
});

// An element picked in the preview becomes a chip in the composer.
watch(
  () => preview.picked,
  (picked) => {
    if (!picked || !composer.value) return;
    composer.value.insertElement(preview.takePicked()!);
  },
);

async function onSubmit(message: PromptInputMessage) {
  if (busy.value) {
    if (canStop.value) await store.stop(props.projectId);
    return;
  }
  const text = message.text;
  if (!text.trim()) return;
  composer.value?.focus();
  await store.send(props.projectId, text);
  const pending = store.takeChangeRequired(props.projectId);
  if (pending) {
    pendingPrompt.value = pending;
    showChangeGuard.value = true;
  }
}

async function onChangeCreated() {
  const text = pendingPrompt.value;
  pendingPrompt.value = null;
  if (text) await store.send(props.projectId, text);
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col" data-testid="chat-panel">
    <div
      v-if="pc?.sessionError"
      class="bg-destructive/10 text-destructive flex items-center justify-between gap-2 border-b px-3 py-2 text-sm"
      role="alert"
      data-testid="session-error"
    >
      <span class="min-w-0 flex-1">Agent session ended: {{ pc.sessionError }}</span>
      <Button size="sm" variant="outline" @click="store.restartSession(projectId)">
        Restart session
      </Button>
    </div>

    <Conversation class="min-h-0">
      <ConversationContent>
        <ConversationEmptyState
          v-if="pc && pc.historyLoaded && pc.turns.length === 0"
          title="No messages yet"
          description="Describe a change to your site and the agent will make it."
        />
        <ChatMessage v-for="turn in pc?.turns" :key="turn.id" :turn="turn" />
        <PermissionPrompt
          v-for="perm in pc?.permissions"
          :key="perm.requestId"
          :permission="perm"
          @allow="store.resolvePermission(projectId, perm.requestId, 'allow')"
          @deny="store.resolvePermission(projectId, perm.requestId, 'deny')"
        />
        <p v-if="pc?.error" class="text-destructive text-sm" role="alert">{{ pc.error }}</p>
      </ConversationContent>
      <ConversationScrollButton />
    </Conversation>

    <div class="shrink-0 p-3">
      <PromptInput
        class="[&_[data-slot=input-group]]:bg-muted/40 [&_[data-slot=input-group]]:rounded-2xl [&_[data-slot=input-group]]:shadow-none [&_[data-slot=input-group]:focus-within]:border-ring"
        @submit="onSubmit"
      >
        <PromptInputBody>
          <ComposerEditor ref="composer" placeholder="Ask for a change…" aria-label="Message" />
        </PromptInputBody>
        <PromptInputFooter class="px-2 pb-2">
          <PromptInputTools class="gap-0.5">
            <DropdownMenu>
              <DropdownMenuTrigger as-child>
                <Button
                  variant="ghost"
                  size="sm"
                  class="text-muted-foreground hover:text-foreground gap-1.5 px-2 font-normal"
                  :disabled="store.providers.length === 0"
                  data-testid="provider"
                >
                  <Sparkle /> {{ provider || "no provider" }} <ChevronDown class="opacity-60" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="start">
                <DropdownMenuRadioGroup
                  :model-value="provider"
                  @update:model-value="(v) => store.setProvider(projectId, String(v))"
                >
                  <DropdownMenuRadioItem v-for="p in store.providers" :key="p.id" :value="p.id">
                    {{ p.id }}
                  </DropdownMenuRadioItem>
                </DropdownMenuRadioGroup>
              </DropdownMenuContent>
            </DropdownMenu>
            <DropdownMenu>
              <DropdownMenuTrigger as-child>
                <Button
                  variant="ghost"
                  size="sm"
                  class="text-muted-foreground hover:text-foreground gap-1.5 px-2 font-normal"
                  data-testid="auto-approve"
                >
                  <component :is="autoApprove ? ShieldCheck : Shield" />
                  {{ autoApprove ? "Auto mode" : "Ask first" }}
                  <ChevronDown class="opacity-60" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="start">
                <DropdownMenuRadioGroup
                  :model-value="autoApprove ? 'auto' : 'ask'"
                  @update:model-value="(v) => (v === 'auto') !== !!autoApprove && emit('toggleAutoApprove')"
                >
                  <DropdownMenuRadioItem value="auto" data-testid="mode-auto">
                    Auto mode
                  </DropdownMenuRadioItem>
                  <DropdownMenuRadioItem value="ask" data-testid="mode-ask">
                    Ask before running tools
                  </DropdownMenuRadioItem>
                </DropdownMenuRadioGroup>
              </DropdownMenuContent>
            </DropdownMenu>
          </PromptInputTools>
          <PromptInputSubmit
            :status="submitStatus"
            :disabled="busy && !canStop"
            :data-testid="busy ? 'stop' : 'send'"
            :aria-label="busy ? 'Stop' : 'Send'"
            class="size-8 rounded-full bg-emerald-600 text-white hover:bg-emerald-500"
          >
            <Square v-if="submitStatus === 'streaming'" class="size-3.5 fill-current" />
            <Loader2 v-else-if="submitStatus === 'submitted'" class="size-4 animate-spin" />
            <ArrowUp v-else class="size-4" />
          </PromptInputSubmit>
        </PromptInputFooter>
      </PromptInput>
    </div>

    <NewChangeDialog
      v-model:open="showChangeGuard"
      :project-id="projectId"
      heading="Start a change first"
      description="GitHub projects cannot be edited on main. Name this change to continue with your prompt."
      @created="onChangeCreated"
    />
  </div>
</template>
