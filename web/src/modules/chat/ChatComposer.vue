<script setup lang="ts">
import { computed, ref } from "vue";
import { ArrowUp, Square } from "@lucide/vue";
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupTextarea } from "@/components/ui/input-group";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";

const props = defineProps<{
  providers: string[];
  /** The chat already has messages: its provider can't change. */
  providerLocked: boolean;
  /** A turn is in flight: offer Stop instead of Send. */
  streaming: boolean;
  /** Why sending is unavailable right now, if it is. */
  blocked: string | null;
}>();
const emit = defineEmits<{ submit: [text: string]; stop: [] }>();
const text = defineModel<string>({ required: true });
const provider = defineModel<string | null>("provider", { required: true });

const labels: Record<string, string> = { cursor: "Cursor", claude: "Claude", codex: "Codex" };
const textarea = ref<{ $el: HTMLTextAreaElement } | null>(null);

const canSend = computed(() => !props.blocked && !!text.value.trim() && !!provider.value);

function submit() {
  if (canSend.value) emit("submit", text.value);
}

function onKeydown(e: KeyboardEvent) {
  if (e.key !== "Enter" || e.shiftKey || e.isComposing) return;
  e.preventDefault();
  submit();
}

defineExpose({ focus: () => textarea.value?.$el.focus() });
</script>

<template>
  <TooltipProvider>
    <InputGroup>
      <InputGroupTextarea
        ref="textarea"
        v-model="text"
        data-testid="chat-input"
        placeholder="Ask the agent to change your site"
        class="max-h-48 min-h-16"
        @keydown="onKeydown"
      />
      <InputGroupAddon align="block-end" class="justify-between">
        <NativeSelect
          v-model="provider"
          data-testid="chat-provider"
          aria-label="Agent"
          class="h-7 py-0 text-xs"
          :disabled="providerLocked"
        >
          <NativeSelectOption v-for="p in providers" :key="p" :value="p">{{ labels[p] ?? p }}</NativeSelectOption>
        </NativeSelect>
        <InputGroupButton
          v-if="streaming"
          variant="default"
          size="icon-xs"
          aria-label="Stop"
          data-testid="chat-stop"
          @click="emit('stop')"
        >
          <Square />
        </InputGroupButton>
        <Tooltip v-else>
          <TooltipTrigger as-child>
            <span tabindex="0" class="rounded-md">
              <InputGroupButton
                variant="default"
                size="icon-xs"
                :aria-label="blocked ?? 'Send'"
                data-testid="chat-send"
                :disabled="!canSend"
                @click="submit"
              >
                <ArrowUp />
              </InputGroupButton>
            </span>
          </TooltipTrigger>
          <TooltipContent>{{ blocked ?? "Send" }}</TooltipContent>
        </Tooltip>
      </InputGroupAddon>
    </InputGroup>
  </TooltipProvider>
</template>
