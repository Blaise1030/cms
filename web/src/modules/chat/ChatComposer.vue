<script setup lang="ts">
import { computed, ref } from "vue";
import { ArrowUp, Check, ChevronDown, ListPlus, MapPin, MousePointerClick, Paperclip, Settings2, ShieldCheck, ShieldQuestion, Square, X } from "@lucide/vue";
import { Badge } from "@/components/ui/badge";
import { pageContextBadge, type PageContext } from "./pageContext";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupTextarea } from "@/components/ui/input-group";
import { Spinner } from "@/components/ui/spinner";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { groupModels, type GroupedModel } from "./modelGroups";
import ProviderIcon from "./ProviderIcon.vue";

export type ChatAttachment = { name: string; path: string };
/** An element picked from the preview, carried with the next message. */
export type ChatReference = { label: string; full: string };

const props = defineProps<{
  providers: string[];
  /** Each assistant's models; missing or empty offers Default only. */
  models: Record<string, { id: string; name: string }[]>;
  /** The chat already has messages: its provider can't change. */
  providerLocked: boolean;
  /** A turn is in flight: offer Stop instead of Send. */
  streaming: boolean;
  /** Why sending is unavailable right now, if it is. */
  blocked: string | null;
  /** Why the input is replaced by a stage line (a sync chat while the site is checked). */
  locked?: string | null;
  /** Files already uploaded for the next message. */
  attachments: ChatAttachment[];
  /** An upload is in flight. */
  attaching: boolean;
  /** Elements picked from the preview for the next message. */
  references: ChatReference[];
  /** Ambient page context that will be sent with the next message. */
  pageContext?: PageContext | null;
  /** Run the assistant's tools without asking. */
  autoApprove: boolean;
}>();
const emit = defineEmits<{
  submit: [text: string];
  stop: [];
  attach: [files: File[]];
  removeAttachment: [index: number];
  removeReference: [index: number];
  clearPageContext: [];
  setAutoApprove: [on: boolean];
  /** The user wants to set up another assistant; the caller opens its setup. */
  setup: [];
}>();
const text = defineModel<string>({ required: true });
const provider = defineModel<string | null>("provider", { required: true });
/** `null` keeps the assistant's own default. */
const model = defineModel<string | null>("model", { required: true });

const labels: Record<string, string> = { cursor: "Cursor", claude: "Claude", codex: "Codex", opencode: "OpenCode" };
/** List items are `{provider}:{model}`, with this standing for the assistant's own default. */
const DEFAULT_MODEL = "default";
const pickerOpen = ref(false);
const labelOf = (p: string) => labels[p] ?? p;
const shownProviders = computed(() => (props.providerLocked && provider.value ? [provider.value] : props.providers));
const choices = (p: string) => [{ id: DEFAULT_MODEL, name: "Default" }, ...(props.models[p] ?? [])];
/** Direct picks vs hover flyouts for `provider/model` ids. */
const menus = computed(() => {
  const out: Record<string, { direct: GroupedModel[]; subs: { heading: string; items: GroupedModel[] }[] }> = {};
  for (const p of shownProviders.value) {
    const direct: GroupedModel[] = [];
    const subs: { heading: string; items: GroupedModel[] }[] = [];
    for (const g of groupModels(choices(p))) {
      if (g.heading === null) direct.push(...g.items);
      else subs.push({ heading: g.heading, items: g.items });
    }
    out[p] = { direct, subs };
  }
  return out;
});
const selected = computed(() => (provider.value ? `${provider.value}:${model.value ?? DEFAULT_MODEL}` : ""));
const pickerLabel = computed(() => {
  if (!provider.value) return "Assistant";
  return props.models[provider.value]?.find((m) => m.id === model.value)?.name ?? labelOf(provider.value);
});
/** Vitest: keep submenu items mounted so picks don't need real hover. */
const forceSubMount = import.meta.env.MODE === "test";

function pickModel(p: string, id: string) {
  provider.value = p;
  model.value = id === DEFAULT_MODEL ? null : id;
  pickerOpen.value = false;
}

/** The setup row is an action, not a pick: close the list and let the caller open setup. */
function setupAnother() {
  pickerOpen.value = false;
  emit("setup");
}
const textarea = ref<{ $el: HTMLTextAreaElement } | null>(null);
const fileInput = ref<HTMLInputElement | null>(null);
const dropping = ref(false);

const canSend = computed(
  () =>
    !props.blocked &&
    !props.locked &&
    !props.attaching &&
    (!!text.value.trim() || props.attachments.length > 0 || props.references.length > 0) &&
    !!provider.value,
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
        Drop files for your assistant
      </div>
      <InputGroup>
        <div
          v-if="attachments.length || attaching || references.length || pageContext"
          class="flex w-full min-w-0 flex-wrap gap-1.5 px-3 pt-2.5"
        >
          <Badge
            v-if="pageContext"
            variant="secondary"
            data-testid="chat-page-context"
            class="max-w-full py-1 pr-2 pl-1 text-xs"
          >
            <button
              type="button"
              class="hover:bg-background shrink-0 rounded p-0.5"
              :aria-label="`Remove ${pageContextBadge(pageContext)}`"
              data-testid="chat-page-context-remove"
              @click="emit('clearPageContext')"
            >
              <X class="size-3" aria-hidden="true" />
            </button>
            <MapPin class="size-3 shrink-0" aria-hidden="true" />
            <span class="min-w-0 truncate">{{ pageContextBadge(pageContext) }}</span>
          </Badge>
          <Badge
            v-for="(reference, i) in references"
            :key="reference.full"
            variant="secondary"
            data-testid="chat-reference"
            class="max-w-full py-1 pr-1 pl-2 text-xs"
          >
            <MousePointerClick class="size-3 shrink-0" aria-hidden="true" />
            <span class="min-w-0 truncate">{{ reference.label }}</span>
            <button
              type="button"
              class="hover:bg-background shrink-0 rounded p-0.5"
              :aria-label="`Remove ${reference.label}`"
              data-testid="chat-reference-remove"
              @click="emit('removeReference', i)"
            >
              <X class="size-3" aria-hidden="true" />
            </button>
          </Badge>
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
        <p
          v-if="locked"
          data-testid="chat-locked"
          class="text-muted-foreground min-h-16 px-3 py-2 text-sm"
        >
          {{ locked }}
        </p>
        <InputGroupTextarea
          v-else
          ref="textarea"
          v-model="text"
          data-testid="chat-input"
          placeholder="Ask your assistant to change your site"
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
            <DropdownMenu v-model:open="pickerOpen">
              <DropdownMenuTrigger as-child>
                <button
                  type="button"
                  data-testid="chat-agent"
                  :data-provider="provider ?? undefined"
                  aria-label="Assistant and model"
                  class="bg-card hover:bg-accent btn-raised-soft dark:bg-input/30 dark:hover:bg-input/50 focus-visible:ring-ring/50 flex h-7 max-w-56 min-w-0 items-center gap-1.5 rounded-md border border-transparent py-0 pr-1.5 pl-2 text-xs whitespace-nowrap outline-none focus-visible:ring-3"
                >
                  <ProviderIcon v-if="provider" :provider="provider" class="size-3.5 text-current" />
                  <span class="min-w-0 truncate">{{ pickerLabel }}</span>
                  <ChevronDown class="text-muted-foreground size-4 shrink-0 opacity-50" />
                </button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="start" class="min-w-48" data-testid="chat-agent-menu">
                <!-- Providers on the first layer; models open in a searchable hover flyout. -->
                <DropdownMenuSub v-for="p in shownProviders" :key="p">
                  <DropdownMenuSubTrigger data-testid="chat-agent-sub" :data-provider="p">
                    <ProviderIcon :provider="p" class="size-3.5 text-current" />
                    <span class="min-w-0 flex-1 truncate">{{ labelOf(p) }}</span>
                    <Check v-if="provider === p" class="size-4" />
                  </DropdownMenuSubTrigger>
                  <DropdownMenuSubContent
                    class="w-72 p-0"
                    :force-mount="forceSubMount || undefined"
                    @close-auto-focus.prevent
                  >
                    <Command :data-testid="`chat-model-menu-${p}`">
                      <CommandInput
                        placeholder="Search models…"
                        data-testid="chat-model-search"
                        @keydown.stop
                      />
                      <CommandList class="max-h-[min(320px,50vh)]">
                        <CommandEmpty>No models found.</CommandEmpty>
                        <CommandGroup v-if="menus[p]?.direct.length">
                          <CommandItem
                            v-for="m in menus[p]?.direct"
                            :key="m.id"
                            :value="`${m.id} ${m.label}`"
                            data-testid="chat-model-item"
                            :data-provider="p"
                            :data-value="`${p}:${m.id}`"
                            @select="pickModel(p, m.id)"
                          >
                            <span data-testid="chat-model-label" class="min-w-0 flex-1 truncate">{{ m.label }}</span>
                            <Check v-if="selected === `${p}:${m.id}`" class="size-4" />
                          </CommandItem>
                        </CommandGroup>
                        <CommandGroup
                          v-for="sub in menus[p]?.subs"
                          :key="`${p}:${sub.heading}`"
                          :heading="sub.heading"
                          data-testid="chat-model-sub"
                          :data-provider="p"
                          :data-model-provider="sub.heading"
                        >
                          <CommandItem
                            v-for="m in sub.items"
                            :key="m.id"
                            :value="`${m.id} ${sub.heading} ${m.label}`"
                            data-testid="chat-model-sub-item"
                            :data-provider="p"
                            :data-value="`${p}:${m.id}`"
                            @select="pickModel(p, m.id)"
                          >
                            <span data-testid="chat-model-label" class="min-w-0 flex-1 truncate">{{ m.label }}</span>
                            <Check v-if="selected === `${p}:${m.id}`" class="size-4" />
                          </CommandItem>
                        </CommandGroup>
                      </CommandList>
                    </Command>
                  </DropdownMenuSubContent>
                </DropdownMenuSub>
                <DropdownMenuSeparator />
                <DropdownMenuItem data-testid="chat-agent-setup" @select="setupAnother">
                  <Settings2 class="size-3.5 text-current" aria-hidden="true" />
                  <span class="min-w-0 flex-1 truncate">Set up another assistant…</span>
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
            <DropdownMenu>
              <DropdownMenuTrigger as-child>
                <button
                  type="button"
                  data-testid="chat-auto-approve"
                  :data-approve="autoApprove ? 'auto' : 'ask'"
                  :aria-label="`Tool approvals: ${autoApprove ? 'Auto-approve' : 'Ask each time'}`"
                  title="Choose whether your assistant asks before running tools"
                  class="bg-card hover:bg-accent btn-raised-soft dark:bg-input/30 dark:hover:bg-input/50 focus-visible:ring-ring/50 flex h-7 shrink-0 items-center gap-1.5 rounded-md border border-transparent py-0 pr-1.5 pl-2 text-xs whitespace-nowrap outline-none focus-visible:ring-3"
                >
                  <ShieldCheck v-if="autoApprove" class="size-3.5" aria-hidden="true" />
                  <ShieldQuestion v-else class="size-3.5" aria-hidden="true" />
                  <span>{{ autoApprove ? "Auto" : "Ask" }}</span>
                  <ChevronDown class="text-muted-foreground size-4 shrink-0 opacity-50" />
                </button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="start" class="w-56">
                <DropdownMenuLabel>Tool approvals</DropdownMenuLabel>
                <DropdownMenuItem data-testid="chat-approval-ask" @select="emit('setAutoApprove', false)">
                  <span class="flex-1">Ask each time</span>
                  <Check v-if="!autoApprove" class="size-4" aria-hidden="true" />
                </DropdownMenuItem>
                <DropdownMenuItem data-testid="chat-approval-auto" @select="emit('setAutoApprove', true)">
                  <span class="flex-1">Auto-approve</span>
                  <Check v-if="autoApprove" class="size-4" aria-hidden="true" />
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
          <div class="flex shrink-0 items-center gap-1">
            <Tooltip>
              <TooltipTrigger as-child>
                <span tabindex="0" class="shrink-0 rounded-md">
                  <InputGroupButton
                    variant="default"
                    size="icon-xs"
                    class="size-7"
                    :aria-label="blocked ?? (streaming ? 'Queue message' : 'Send')"
                    :data-testid="streaming ? 'chat-queue' : 'chat-send'"
                    :disabled="!canSend"
                    @click="submit"
                  >
                    <ListPlus v-if="streaming" />
                    <ArrowUp v-else />
                  </InputGroupButton>
                </span>
              </TooltipTrigger>
              <TooltipContent>{{ blocked ?? (streaming ? "Queue message" : "Send") }}</TooltipContent>
            </Tooltip>
            <InputGroupButton
              v-if="streaming"
              variant="ghost"
              size="icon-xs"
              class="size-7 shrink-0"
              aria-label="Stop"
              data-testid="chat-stop"
              @click="emit('stop')"
            >
              <Square />
            </InputGroupButton>
          </div>
        </InputGroupAddon>
      </InputGroup>
    </div>
  </TooltipProvider>
</template>
