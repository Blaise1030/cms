<script setup lang="ts">
import { computed, ref } from "vue";
import { ArrowUp, Check, ChevronDown, Paperclip, Square, X } from "@lucide/vue";
import { Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList } from "@/components/ui/command";
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupTextarea } from "@/components/ui/input-group";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Spinner } from "@/components/ui/spinner";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import ProviderIcon from "./ProviderIcon.vue";

export type ChatAttachment = { name: string; path: string };

const props = defineProps<{
  providers: string[];
  /** Each agent's models; missing or empty offers Default only. */
  models: Record<string, { id: string; name: string }[]>;
  /** The chat already has messages: its provider can't change. */
  providerLocked: boolean;
  /** A turn is in flight: offer Stop instead of Send. */
  streaming: boolean;
  /** Why sending is unavailable right now, if it is. */
  blocked: string | null;
  /** Files already uploaded for the next message. */
  attachments: ChatAttachment[];
  /** An upload is in flight. */
  attaching: boolean;
}>();
const emit = defineEmits<{
  submit: [text: string];
  stop: [];
  attach: [files: File[]];
  removeAttachment: [index: number];
}>();
const text = defineModel<string>({ required: true });
const provider = defineModel<string | null>("provider", { required: true });
/** `null` keeps the agent's own default. */
const model = defineModel<string | null>("model", { required: true });

const labels: Record<string, string> = { cursor: "Cursor", claude: "Claude", codex: "Codex", opencode: "OpenCode" };
/** List items are `{provider}:{model}`, with this standing for the agent's own default. */
const DEFAULT_MODEL = "default";
const pickerOpen = ref(false);
const labelOf = (p: string) => labels[p] ?? p;
const shownProviders = computed(() => (props.providerLocked && provider.value ? [provider.value] : props.providers));
const choices = (p: string) => [{ id: DEFAULT_MODEL, name: "Default" }, ...(props.models[p] ?? [])];
const selected = computed(() => (provider.value ? `${provider.value}:${model.value ?? DEFAULT_MODEL}` : ""));
const pickerLabel = computed(() => {
  if (!provider.value) return "Agent";
  return props.models[provider.value]?.find((m) => m.id === model.value)?.name ?? labelOf(provider.value);
});

function pick(value: unknown) {
  if (typeof value !== "string") return;
  const at = value.indexOf(":");
  if (at < 0) return;
  const id = value.slice(at + 1);
  provider.value = value.slice(0, at);
  model.value = id === DEFAULT_MODEL ? null : id;
  pickerOpen.value = false;
}
const textarea = ref<{ $el: HTMLTextAreaElement } | null>(null);
const fileInput = ref<HTMLInputElement | null>(null);
const dropping = ref(false);

const canSend = computed(
  () => !props.streaming && !props.blocked && !props.attaching && (!!text.value.trim() || props.attachments.length > 0) && !!provider.value,
);

function submit() {
  if (canSend.value) emit("submit", text.value);
}

function onKeydown(e: KeyboardEvent) {
  if (e.key !== "Enter" || e.shiftKey || e.isComposing) return;
  e.preventDefault();
  submit();
}

function pickFiles() {
  fileInput.value?.click();
}

function onFileChange(e: Event) {
  const input = e.target as HTMLInputElement;
  if (input.files?.length) emit("attach", [...input.files]);
  input.value = "";
}

function onPaste(e: ClipboardEvent) {
  const files = [...(e.clipboardData?.files ?? [])];
  if (!files.length) return;
  e.preventDefault();
  emit("attach", files);
}

const carriesFiles = (e: DragEvent) => e.dataTransfer?.types.includes("Files") ?? false;

function onDragOver(e: DragEvent) {
  if (!carriesFiles(e)) return;
  e.preventDefault();
  dropping.value = true;
}

function onDragLeave(e: DragEvent) {
  if (!(e.currentTarget as Element).contains(e.relatedTarget as Node | null)) dropping.value = false;
}

function onDrop(e: DragEvent) {
  if (!carriesFiles(e)) return;
  e.preventDefault();
  dropping.value = false;
  const files = [...(e.dataTransfer?.files ?? [])];
  if (files.length) emit("attach", files);
}

defineExpose({ focus: () => textarea.value?.$el.focus() });
</script>

<template>
  <TooltipProvider>
    <div
      class="relative"
      data-testid="chat-composer"
      @dragover="onDragOver"
      @dragleave="onDragLeave"
      @drop="onDrop"
    >
      <div
        v-if="dropping"
        class="bg-background/80 pointer-events-none absolute inset-0 z-10 flex items-center justify-center rounded-xl border border-dashed text-sm"
        data-testid="chat-drop-overlay"
      >
        Drop files for the agent
      </div>
      <InputGroup>
        <div v-if="attachments.length || attaching" class="flex w-full min-w-0 flex-wrap gap-1.5 px-3 pt-2.5">
          <span
            v-for="(a, i) in attachments"
            :key="a.path"
            data-testid="chat-attachment"
            class="bg-muted inline-flex max-w-full items-center gap-1.5 rounded-md py-1 pr-1 pl-2 text-xs"
          >
            <Paperclip class="size-3 shrink-0" aria-hidden="true" />
            <span class="min-w-0 truncate">{{ a.name }}</span>
            <button
              type="button"
              class="hover:bg-background shrink-0 rounded p-0.5"
              :aria-label="`Remove ${a.name}`"
              @click="emit('removeAttachment', i)"
            >
              <X class="size-3" aria-hidden="true" />
            </button>
          </span>
          <span v-if="attaching" class="text-muted-foreground inline-flex items-center gap-1.5 py-1 text-xs">
            <Spinner class="size-3.5" aria-hidden="true" /> Attaching…
          </span>
        </div>
        <InputGroupTextarea
          ref="textarea"
          v-model="text"
          data-testid="chat-input"
          placeholder="Ask the agent to change your site"
          class="max-h-48 min-h-16"
          @keydown="onKeydown"
          @paste="onPaste"
        />
        <InputGroupAddon align="block-end" class="justify-between">
          <div class="flex min-w-0 items-center gap-2">
            <input
              ref="fileInput"
              type="file"
              multiple
              class="hidden"
              data-testid="chat-attach-input"
              accept="image/png,image/jpeg,image/gif,image/webp,application/pdf,text/plain,text/markdown,text/csv,application/json,.md"
              @change="onFileChange"
            />
            <InputGroupButton
              variant="ghost"
              size="icon-xs"
              class="size-7 shrink-0"
              aria-label="Attach image or file"
              data-testid="chat-attach"
              :disabled="attaching"
              @click="pickFiles"
            >
              <Paperclip />
            </InputGroupButton>
            <Popover v-model:open="pickerOpen">
              <PopoverTrigger as-child>
                <button
                  type="button"
                  data-testid="chat-agent"
                  :data-provider="provider ?? undefined"
                  aria-label="Agent and model"
                  class="bg-card hover:bg-accent btn-raised-soft dark:bg-input/30 dark:hover:bg-input/50 focus-visible:ring-ring/50 flex h-7 max-w-56 min-w-0 items-center gap-1.5 rounded-md border border-transparent py-0 pr-1.5 pl-2 text-xs whitespace-nowrap outline-none focus-visible:ring-3"
                >
                  <ProviderIcon v-if="provider" :provider="provider" class="size-3.5 text-current" />
                  <span class="min-w-0 truncate">{{ pickerLabel }}</span>
                  <ChevronDown class="text-muted-foreground size-4 shrink-0 opacity-50" />
                </button>
              </PopoverTrigger>
              <PopoverContent align="start" class="w-72 p-0">
                <Command :model-value="selected" @update:model-value="pick">
                  <CommandInput placeholder="Search agents and models" data-testid="chat-agent-search" />
                  <CommandList class="max-h-80 p-1">
                    <CommandEmpty>No models found</CommandEmpty>
                    <CommandGroup
                      v-for="p in shownProviders"
                      :key="p"
                      :heading="labelOf(p)"
                      :data-provider="p"
                      data-testid="chat-agent-group"
                      class="p-0"
                    >
                      <CommandItem
                        v-for="m in choices(p)"
                        :key="m.id"
                        :value="`${p}:${m.id}`"
                        :data-value="`${p}:${m.id}`"
                        data-testid="chat-model-item"
                      >
                        <ProviderIcon :provider="p" class="size-3.5 text-current" />
                        <span class="min-w-0 flex-1 truncate">{{ m.name }}</span>
                        <span class="sr-only">{{ labelOf(p) }}</span>
                        <Check v-if="selected === `${p}:${m.id}`" class="size-4" />
                      </CommandItem>
                    </CommandGroup>
                  </CommandList>
                </Command>
              </PopoverContent>
            </Popover>
          </div>
          <InputGroupButton
            v-if="streaming"
            variant="default"
            size="icon-xs"
            class="size-7 shrink-0"
            aria-label="Stop"
            data-testid="chat-stop"
            @click="emit('stop')"
          >
            <Square />
          </InputGroupButton>
          <Tooltip v-else>
            <TooltipTrigger as-child>
              <span tabindex="0" class="shrink-0 rounded-md">
                <InputGroupButton
                  variant="default"
                  size="icon-xs"
                  class="size-7"
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
    </div>
  </TooltipProvider>
</template>
