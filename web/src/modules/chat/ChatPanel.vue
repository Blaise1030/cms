<script setup lang="ts">
import { computed, onMounted, watch } from "vue";
import { Bot, ChevronDown } from "@lucide/vue";
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
  PromptInputTextarea,
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
import ChatMessage from "./ChatMessage.vue";
import PermissionPrompt from "./PermissionPrompt.vue";
import { useChatStore } from "./store";

const props = defineProps<{ projectId: string; autoApprove?: boolean }>();
const emit = defineEmits<{ toggleAutoApprove: [] }>();

const store = useChatStore();
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

async function onSubmit(message: PromptInputMessage) {
  if (busy.value) {
    if (canStop.value) await store.stop(props.projectId);
    return;
  }
  const text = message.text;
  if (!text.trim()) return;
  await store.send(props.projectId, text);
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

    <div class="shrink-0 border-t p-3">
      <PromptInput class="rounded-xl" @submit="onSubmit">
        <PromptInputBody>
          <PromptInputTextarea
            placeholder="Ask for a change…"
            aria-label="Message"
          />
        </PromptInputBody>
        <PromptInputFooter>
          <PromptInputTools>
            <DropdownMenu>
              <DropdownMenuTrigger as-child>
                <Button variant="ghost" size="sm" :disabled="store.providers.length === 0">
                  <Bot /> {{ provider || "no provider" }} <ChevronDown />
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
            <Button
              variant="ghost"
              size="sm"
              :aria-pressed="!!autoApprove"
              data-testid="auto-approve"
              @click="emit('toggleAutoApprove')"
            >
              Auto-approve: {{ autoApprove ? "on" : "off" }}
            </Button>
          </PromptInputTools>
          <PromptInputSubmit
            :status="submitStatus"
            :disabled="busy && !canStop"
            :data-testid="busy ? 'stop' : 'send'"
            :aria-label="busy ? 'Stop' : 'Send'"
          />
        </PromptInputFooter>
      </PromptInput>
    </div>
  </div>
</template>
