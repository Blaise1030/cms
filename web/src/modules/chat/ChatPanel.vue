<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { Bot, ChevronDown, SendHorizontal, Square } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Textarea } from "@/components/ui/textarea";
import ChatMessage from "./ChatMessage.vue";
import PermissionPrompt from "./PermissionPrompt.vue";
import { useChatStore } from "./store";

const props = defineProps<{ projectId: string; autoApprove?: boolean }>();
const emit = defineEmits<{ toggleAutoApprove: [] }>();

const store = useChatStore();
const pc = computed(() => store.byProject[props.projectId]);
const busy = computed(() => store.isBusy(props.projectId));
const canStop = computed(() => !!pc.value?.turns.some((t) => t.status === "streaming" && !t.pending));
const draft = ref("");
const root = ref<HTMLElement | null>(null);
let stickToBottom = true;

const provider = computed(() => pc.value?.provider ?? store.providers[0]?.id ?? "");

watch(
  () => props.projectId,
  (id) => {
    stickToBottom = true;
    void store.openProject(id);
  },
  { immediate: true },
);

function viewport(): HTMLElement | null {
  return root.value?.querySelector<HTMLElement>("[data-slot='scroll-area-viewport']") ?? null;
}

onMounted(() => {
  viewport()?.addEventListener("scroll", () => {
    const el = viewport();
    if (el) stickToBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
  });
});

// Follow the stream while the user is at the bottom.
watch(
  () => {
    const c = pc.value;
    const last = c?.turns[c.turns.length - 1];
    return [c?.turns.length, last?.assistantText.length, last?.tools.length, c?.permissions.length];
  },
  async () => {
    await nextTick();
    const el = viewport();
    if (el && stickToBottom) el.scrollTop = el.scrollHeight;
  },
  { flush: "post" },
);

async function submit() {
  const text = draft.value;
  if (!text.trim() || busy.value) return;
  draft.value = "";
  stickToBottom = true;
  await store.send(props.projectId, text);
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
    e.preventDefault();
    void submit();
  }
}
</script>

<template>
  <div ref="root" class="flex h-full min-h-0 flex-col" data-testid="chat-panel">
    <div class="flex h-10 shrink-0 items-center justify-between gap-2 border-b px-3">
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
    </div>

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

    <ScrollArea class="min-h-0 flex-1">
      <div class="flex flex-col gap-5 p-3">
        <p
          v-if="pc && pc.historyLoaded && pc.turns.length === 0"
          class="text-muted-foreground py-8 text-center text-sm"
        >
          Describe a change to your site and the agent will make it.
        </p>
        <ChatMessage v-for="turn in pc?.turns" :key="turn.id" :turn="turn" />
        <PermissionPrompt
          v-for="perm in pc?.permissions"
          :key="perm.requestId"
          :permission="perm"
          @allow="store.resolvePermission(projectId, perm.requestId, 'allow')"
          @deny="store.resolvePermission(projectId, perm.requestId, 'deny')"
        />
        <p v-if="pc?.error" class="text-destructive text-sm" role="alert">{{ pc.error }}</p>
      </div>
    </ScrollArea>

    <form class="flex shrink-0 items-end gap-2 border-t p-3" @submit.prevent="submit">
      <Textarea
        v-model="draft"
        class="max-h-40 min-h-10 resize-none"
        placeholder="Ask for a change…"
        aria-label="Message"
        :rows="2"
        @keydown="onKeydown"
      />
      <Button
        v-if="busy"
        type="button"
        variant="destructive"
        :disabled="!canStop"
        data-testid="stop"
        @click="store.stop(projectId)"
      >
        <Square /> Stop
      </Button>
      <Button v-else type="submit" :disabled="!draft.trim()" data-testid="send" aria-label="Send">
        <SendHorizontal />
      </Button>
    </form>
  </div>
</template>
