<script setup lang="ts">
import { computed, ref } from "vue";
import { TooltipProvider } from "reka-ui";
import type { DropdownMenuItem as MenuItem } from "@nuxt/ui";
import UBadge from "@nuxt/ui/components/Badge.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UChatPrompt from "@nuxt/ui/components/ChatPrompt.vue";
import UChatPromptSubmit from "@nuxt/ui/components/ChatPromptSubmit.vue";
import UDropdownMenu from "@nuxt/ui/components/DropdownMenu.vue";
import UFileUpload from "@nuxt/ui/components/FileUpload.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import UTextarea from "@nuxt/ui/components/Textarea.vue";
import UTooltip from "@nuxt/ui/components/Tooltip.vue";
import { pageContextBadge, viewportBadge, type PageContext } from "./pageContext";
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
const VIEWPORT_ICON: Record<string, string> = {
  desktop: "i-hugeicons-computer",
  tablet: "i-hugeicons-tablet-01",
  mobile: "i-hugeicons-smart-phone-01",
};

const emit = defineEmits<{
  submit: [text: string];
  stop: [];
  attach: [files: File[]];
  removeAttachment: [index: number];
  removeReference: [index: number];
  clearPageContext: [];
  clearViewport: [];
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
const selected = computed(() => (provider.value ? `${provider.value}:${model.value ?? DEFAULT_MODEL}` : ""));
const pickerLabel = computed(() => {
  if (!provider.value) return "Assistant";
  return props.models[provider.value]?.find((m) => m.id === model.value)?.name ?? labelOf(provider.value);
});
/** Vitest: open every provider flyout with the menu so picks don't need real hover. */
const forceSubMount = import.meta.env.MODE === "test";

/** One model row; `data-*` keys land on the menu item element (Nuxt UI forwards them). */
function modelItem(p: string, m: GroupedModel, testid: string, search: string): MenuItem {
  return {
    label: m.label,
    slot: "model" as const,
    search,
    selected: selected.value === `${p}:${m.id}`,
    "data-testid": testid,
    "data-provider": p,
    "data-value": `${p}:${m.id}`,
    onSelect: () => pickModel(p, m.id),
  };
}

/** A provider's flyout: direct picks first, then one labelled group per `provider/model` prefix. */
function modelGroups(p: string): MenuItem[][] {
  const direct: MenuItem[] = [];
  const groups: MenuItem[][] = [];
  for (const g of groupModels(choices(p))) {
    if (g.heading === null) {
      direct.push(...g.items.map((m) => modelItem(p, m, "chat-model-item", `${m.id} ${m.label}`)));
    } else {
      const heading = g.heading;
      groups.push([
        { type: "label", label: heading, slot: "model-group" as const, provider: p },
        ...g.items.map((m) => modelItem(p, m, "chat-model-sub-item", `${m.id} ${heading} ${m.label}`)),
      ]);
    }
  }
  return direct.length ? [direct, ...groups] : groups;
}

/** Slot props of custom-slot rows (Nuxt UI types them from literal slot names only). */
const row = (item: unknown) => item as MenuItem;

/** Providers on the first layer; each opens a searchable flyout of its models. */
const pickerItems = computed<MenuItem[][]>(() => [
  shownProviders.value.map((p) => ({
    label: labelOf(p),
    slot: "provider" as const,
    provider: p,
    children: modelGroups(p),
    filter: { placeholder: "Search models…", "data-testid": "chat-model-search" },
    filterFields: ["search"],
    open: forceSubMount || undefined,
    ui: { content: "w-72" },
    content: { onCloseAutoFocus: (e: Event) => e.preventDefault() },
  })),
  [
    {
      label: "Set up another assistant…",
      icon: "i-hugeicons-settings-04",
      "data-testid": "chat-agent-setup",
      onSelect: setupAnother,
    },
  ],
]);

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
/** Tool approvals: an ask/auto pair; the mode in force shows a check (see `#item-trailing`). */
const approvalItems = computed<MenuItem[][]>(() => [
  [{ type: "label", label: "Tool approvals" }],
  [
    {
      label: "Ask each time",
      active: !props.autoApprove,
      "data-testid": "chat-approval-ask",
      onSelect: () => emit("setAutoApprove", false),
    },
    {
      label: "Auto-approve",
      active: props.autoApprove,
      "data-testid": "chat-approval-auto",
      onSelect: () => emit("setAutoApprove", true),
    },
  ],
]);
const textarea = ref<{ textareaRef: HTMLTextAreaElement | null } | null>(null);
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

function onPicked(picked: unknown[] | null | undefined) {
  const files = (picked ?? []).filter((f): f is File => f instanceof File);
  if (files.length) emit("attach", files);
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

defineExpose({ focus: () => textarea.value?.textareaRef?.focus() });
</script>

<template>
  <!-- UApp provides this in the app; kept here so the composer also mounts on its own. -->
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
        class="bg-default/80 border-accented pointer-events-none absolute inset-0 z-10 flex items-center justify-center rounded-lg border border-dashed text-sm"
        data-testid="chat-drop-overlay"
      >
        Drop files for your assistant
      </div>
      <!-- `as="div"`: sending is driven by our own keydown/buttons (attachments-only sends, queueing). -->
      <UChatPrompt as="div" variant="outline" :ui="{ header: 'flex-wrap' }">
        <template v-if="attachments.length || attaching || references.length || pageContext" #header>
          <UBadge
            v-if="pageContext"
            color="neutral"
            variant="soft"
            data-testid="chat-page-context"
            class="max-w-full gap-1 pl-0.5"
          >
            <UButton
              color="neutral"
              variant="ghost"
              size="xs"
              icon="i-hugeicons-cancel-01"
              class="shrink-0 p-0.5"
              :ui="{ leadingIcon: 'size-3' }"
              :aria-label="`Remove ${pageContextBadge(pageContext)}`"
              data-testid="chat-page-context-remove"
              @click="emit('clearPageContext')"
            />
            <UIcon name="i-hugeicons-location-01" class="size-3 shrink-0" aria-hidden="true" />
            <span class="min-w-0 truncate">{{ pageContextBadge(pageContext) }}</span>
          </UBadge>
          <UBadge
            v-if="viewportBadge(pageContext ?? null)"
            color="neutral"
            variant="soft"
            data-testid="chat-device"
            class="max-w-full gap-1 pl-0.5"
          >
            <UButton
              color="neutral"
              variant="ghost"
              size="xs"
              icon="i-hugeicons-cancel-01"
              class="shrink-0 p-0.5"
              :ui="{ leadingIcon: 'size-3' }"
              :aria-label="`Remove ${viewportBadge(pageContext ?? null)}`"
              data-testid="chat-device-remove"
              @click="emit('clearViewport')"
            />
            <UIcon :name="VIEWPORT_ICON[pageContext?.fields.viewport?.split(' ')[0] ?? ''] ?? 'i-hugeicons-computer'" class="size-3 shrink-0" aria-hidden="true" />
            <span class="min-w-0 truncate">{{ viewportBadge(pageContext ?? null) }}</span>
          </UBadge>
          <UBadge
            v-for="(reference, i) in references"
            :key="reference.full"
            color="neutral"
            variant="soft"
            data-testid="chat-reference"
            class="max-w-full gap-1 pr-0.5"
          >
            <UIcon name="i-hugeicons-cursor-pointer-01" class="size-3 shrink-0" aria-hidden="true" />
            <span class="min-w-0 truncate">{{ reference.label }}</span>
            <UButton
              color="neutral"
              variant="ghost"
              size="xs"
              icon="i-hugeicons-cancel-01"
              class="shrink-0 p-0.5"
              :ui="{ leadingIcon: 'size-3' }"
              :aria-label="`Remove ${reference.label}`"
              data-testid="chat-reference-remove"
              @click="emit('removeReference', i)"
            />
          </UBadge>
          <UBadge
            v-for="(a, i) in attachments"
            :key="a.path"
            color="neutral"
            variant="outline"
            data-testid="chat-attachment"
            class="max-w-full gap-1 pr-0.5"
          >
            <UIcon name="i-hugeicons-attachment-01" class="size-3 shrink-0" aria-hidden="true" />
            <span class="min-w-0 truncate">{{ a.name }}</span>
            <UButton
              color="neutral"
              variant="ghost"
              size="xs"
              icon="i-hugeicons-cancel-01"
              class="shrink-0 p-0.5"
              :ui="{ leadingIcon: 'size-3' }"
              :aria-label="`Remove ${a.name}`"
              @click="emit('removeAttachment', i)"
            />
          </UBadge>
          <span v-if="attaching" class="text-muted inline-flex items-center gap-1.5 py-1 text-xs">
            <UIcon name="i-hugeicons-loading-03" class="size-3.5 animate-spin" aria-hidden="true" /> Attaching…
          </span>
        </template>

        <template #body>
          <p
            v-if="locked"
            data-testid="chat-locked"
            class="text-muted min-h-10 py-1 text-sm"
          >
            {{ locked }}
          </p>
          <UTextarea
            v-else
            ref="textarea"
            v-model="text"
            variant="none"
            :rows="1"
            autoresize
            :maxrows="8"
            data-testid="chat-input"
            placeholder="Ask your assistant to change your site"
            class="w-full"
            :ui="{ base: 'max-h-48 min-h-10 px-0 py-1' }"
            @keydown="onKeydown"
            @paste="onPaste"
          />
        </template>

        <template #footer>
          <div class="flex min-w-0 items-center gap-2">
            <!-- The picker only: the whole composer is the drop target. -->
            <UFileUpload
              v-slot="{ open }"
              multiple
              reset
              :dropzone="false"
              :model-value="[]"
              accept="image/png,image/jpeg,image/gif,image/webp,application/pdf,text/plain,text/markdown,text/csv,application/json,.md"
              class="shrink-0"
              data-testid="chat-attach-input"
              @update:model-value="onPicked"
            >
              <UButton
                color="neutral"
                variant="ghost"
                size="sm"
                icon="i-hugeicons-attachment-01"
                class="shrink-0"
                aria-label="Attach image or file"
                data-testid="chat-attach"
                :disabled="attaching"
                @click="open()"
              />
            </UFileUpload>
            <UDropdownMenu
              v-model:open="pickerOpen"
              :items="pickerItems"
              :content="{ align: 'start' }"
              :ui="{ content: 'min-w-48' }"
            >
              <UButton
                color="neutral"
                variant="outline"
                size="xs"
                trailing-icon="i-hugeicons-arrow-down-01"
                data-testid="chat-agent"
                :data-provider="provider ?? undefined"
                aria-label="Assistant and model"
                class="h-7 max-w-56 min-w-0 whitespace-nowrap"
                :ui="{ trailingIcon: 'text-dimmed' }"
              >
                <ProviderIcon v-if="provider" :provider="provider" class="size-3.5 text-current" />
                <span class="min-w-0 truncate">{{ pickerLabel }}</span>
              </UButton>
              <template #provider-leading="{ item }">
                <ProviderIcon :provider="row(item).provider" class="size-3.5 text-current" />
              </template>
              <template #provider-label="{ item }">
                <!-- Sub triggers take no attrs; the label carries the flyout's testid (hovering it opens the flyout). -->
                <span data-testid="chat-agent-sub" :data-provider="row(item).provider">{{ row(item).label }}</span>
              </template>
              <template #provider-trailing="{ item }">
                <UIcon v-if="provider === row(item).provider" name="i-hugeicons-tick-02" class="size-4" aria-hidden="true" />
                <UIcon name="i-hugeicons-arrow-right-01" class="text-dimmed size-4" aria-hidden="true" />
              </template>
              <template #model-group="{ item }">
                <span data-testid="chat-model-sub" :data-provider="row(item).provider" :data-model-provider="row(item).label">
                  {{ row(item).label }}
                </span>
              </template>
              <template #model-label="{ item }">
                <span data-testid="chat-model-label">{{ row(item).label }}</span>
              </template>
              <template #model-trailing="{ item }">
                <UIcon v-if="row(item).selected" name="i-hugeicons-tick-02" class="size-4" aria-hidden="true" />
              </template>
            </UDropdownMenu>
            <UDropdownMenu :items="approvalItems" :content="{ align: 'start' }" :ui="{ content: 'w-56' }">
              <UButton
                color="neutral"
                variant="outline"
                size="xs"
                :icon="autoApprove ? 'i-hugeicons-shield-check' : 'i-hugeicons-shield-01'"
                trailing-icon="i-hugeicons-arrow-down-01"
                :label="autoApprove ? 'Auto' : 'Ask'"
                data-testid="chat-auto-approve"
                :data-approve="autoApprove ? 'auto' : 'ask'"
                :aria-label="`Tool approvals: ${autoApprove ? 'Auto-approve' : 'Ask each time'}`"
                title="Choose whether your assistant asks before running tools"
                class="h-7 shrink-0 whitespace-nowrap"
                :ui="{ trailingIcon: 'text-dimmed' }"
              />
              <template #item-trailing="{ item }">
                <UIcon v-if="item.active" name="i-hugeicons-tick-02" class="size-4" aria-hidden="true" />
              </template>
            </UDropdownMenu>
          </div>
          <div class="flex shrink-0 items-center gap-1">
            <UTooltip :text="blocked ?? (streaming ? 'Queue message' : 'Send')">
              <!-- The span keeps the tooltip reachable while the button is disabled. -->
              <span tabindex="0" class="shrink-0 rounded-md">
                <!-- While a turn runs, a new message is queued (UChatPromptSubmit has no queue state). -->
                <UButton
                  v-if="streaming"
                  size="sm"
                  icon="i-hugeicons-add-to-list"
                  :aria-label="blocked ?? 'Queue message'"
                  data-testid="chat-queue"
                  :disabled="!canSend"
                  @click="submit"
                />
                <UChatPromptSubmit
                  v-else
                  status="ready"
                  size="sm"
                  icon="i-hugeicons-arrow-up-01"
                  :aria-label="blocked ?? 'Send'"
                  data-testid="chat-send"
                  :disabled="!canSend"
                  @click="submit"
                />
              </span>
            </UTooltip>
            <UChatPromptSubmit
              v-if="streaming"
              status="streaming"
              size="sm"
              streaming-icon="i-hugeicons-stop"
              class="shrink-0"
              aria-label="Stop"
              data-testid="chat-stop"
              @stop="emit('stop')"
            />
          </div>
        </template>
      </UChatPrompt>
    </div>
  </TooltipProvider>
</template>
