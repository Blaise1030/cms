<script setup lang="ts">
import { computed, reactive } from "vue";
import type { DynamicToolUIPart, UIMessage } from "ai";
import { Markdown, type StreamMarkdownParserOptions } from "vue-stream-markdown";
import "vue-stream-markdown/index.css";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UBadge from "@nuxt/ui/components/Badge.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UCollapsible from "@nuxt/ui/components/Collapsible.vue";
import ULink from "@nuxt/ui/components/Link.vue";
import UChatMessage from "@nuxt/ui/components/ChatMessage.vue";
import UChatReasoning from "@nuxt/ui/components/ChatReasoning.vue";
import UChatTool from "@nuxt/ui/components/ChatTool.vue";
import { useDaemon } from "@/core/daemon";
import { useChatSteps } from "@/lib/chatSteps";
import ChatCheckpoint from "./ChatCheckpoint.vue";
import { splitAttachments } from "./attachments";
import { openWebLink } from "./links";
import { pageContextBadge, splitPageContext, viewportBadge } from "./pageContext";
import {
  formatValue,
  groupSteps,
  stepsTitle,
  syncIsTerminal,
  syncPhaseData,
  syncPhaseOf,
  toolLabel,
  type AgentErrorData,
  type CheckpointData,
  type MessageGroup,
  type ToolInput,
} from "./parts";
import { segmentReferences } from "./references";

const props = defineProps<{
  projectId: string;
  message: UIMessage;
  /** The assistant behind the open chat; named by recovery actions. */
  provider?: string | null;
}>();
const emit = defineEmits<{
  assistantAction: [{ action: "setup" | "signIn" | "retry"; provider: string }];
}>();
const daemon = useDaemon();
const chatSteps = useChatSteps();

/** The user's text parts, with attachment paths and page-context blocks split back out. */
const userTexts = computed(() =>
  props.message.parts.flatMap((p) => {
    if (p.type !== "text") return [];
    const attached = splitAttachments(p.text);
    const { text, context } = splitPageContext(attached.text);
    return [{ text, files: attached.files, context }];
  }),
);
const groups = computed(() => groupSteps(props.message.parts));
/** Attached images that failed to load (e.g. a cross-site daemon); shown as file chips. */
const broken = reactive(new Set<string>());

/** Finished steps follow Appearance; in-flight “Working…” always stays visible. */
const showStepsGroup = (group: Extract<MessageGroup, { kind: "steps" }>) =>
  group.working || chatSteps.enabled.value;

/** Agent text is untrusted: replacing the parser's default `html` plugin leaves raw HTML as escaped text. */
const parserOptions: StreamMarkdownParserOptions = { plugins: [{ name: "html" }] };

const toolIcons: Record<string, string> = {
  read: "i-hugeicons-file-01",
  edit: "i-hugeicons-file-edit",
  shell: "i-hugeicons-computer-terminal-01",
  search: "i-hugeicons-search-01",
  web: "i-hugeicons-globe-02",
};

const toolStatus = (part: DynamicToolUIPart) =>
  part.state === "output-error" ? "error" : part.state === "output-available" ? undefined : "running";
/** The agent's own arguments, when the daemon kept them. */
const toolArgs = (part: DynamicToolUIPart) => {
  const input = (part.input ?? {}) as ToolInput;
  return input.raw ?? input;
};

const checkpoint = (data: unknown) => data as CheckpointData;
const agentError = (data: unknown) => data as AgentErrorData;

/** The card's plain-language headline; the daemon's raw message stays in details. */
const ERROR_TITLE: Record<AgentErrorData["code"], string> = {
  not_installed: "Your assistant isn’t set up",
  login: "Your assistant needs you to sign in",
  failed: "Your assistant ran into a problem",
};

/** Tool payloads: monospace, scrolls instead of stretching the transcript. */
const codeClass =
  "bg-muted text-default border-default max-h-48 overflow-auto rounded-md border px-2 py-1.5 font-mono text-xs whitespace-pre-wrap break-words";

function act(action: "setup" | "signIn" | "retry", data: unknown) {
  emit("assistantAction", { action, provider: agentError(data).provider ?? props.provider ?? "" });
}
</script>

<template>
  <!-- One user turn: each image, the file chips and the text are separate right-aligned bubbles. -->
  <div v-if="message.role === 'user'" class="flex w-full flex-col items-end gap-1.5">
    <template v-for="(sent, i) in userTexts" :key="i">
      <template v-for="f in sent.files" :key="f.name">
        <UChatMessage
          v-if="f.image && !broken.has(f.name)"
          :id="`${message.id}-${i}-${f.name}`"
          role="user"
          :parts="[]"
          side="right"
          variant="soft"
          compact
          data-testid="chat-bubble"
          :ui="{ container: 'pb-0', content: 'overflow-hidden p-1' }"
        >
          <template #content>
            <ULink
              data-testid="message-attachment"
              :to="daemon.api.attachmentUrl(projectId, f.name)"
              external
              target="_blank"
              rel="noopener noreferrer"
              raw
              class="block overflow-hidden rounded-lg"
            >
              <img
                :src="daemon.api.attachmentUrl(projectId, f.name)"
                :alt="`Attached ${f.kind} image`"
                class="max-h-48 max-w-full object-contain"
                @error="broken.add(f.name)"
              />
            </ULink>
          </template>
        </UChatMessage>
      </template>
      <UChatMessage
        v-if="sent.files.some((f) => !f.image || broken.has(f.name))"
        :id="`${message.id}-${i}-files`"
        role="user"
        :parts="[]"
        side="right"
        variant="soft"
        compact
        data-testid="chat-bubble"
        :ui="{ container: 'pb-0', content: 'flex flex-wrap justify-end gap-1.5 space-y-0' }"
      >
        <template #content>
          <template v-for="f in sent.files" :key="f.name">
            <UBadge
              v-if="!f.image || broken.has(f.name)"
              color="neutral"
              variant="outline"
              icon="i-hugeicons-attachment-01"
              :label="`${f.kind} file`"
              data-testid="message-attachment"
            />
          </template>
        </template>
      </UChatMessage>
      <UBadge
        v-if="sent.context"
        color="neutral"
        variant="soft"
        data-testid="message-page-context"
        class="max-w-full"
        :label="pageContextBadge(sent.context)"
      />
      <UBadge
        v-if="viewportBadge(sent.context)"
        color="neutral"
        variant="soft"
        data-testid="message-viewport"
        class="max-w-full"
        :label="viewportBadge(sent.context)!"
      />
      <UChatMessage
        v-if="sent.text"
        :id="`${message.id}-${i}-text`"
        role="user"
        :parts="[]"
        side="right"
        variant="soft"
        compact
        data-testid="chat-bubble"
        :ui="{ container: 'pb-0', content: 'text-sm whitespace-pre-wrap' }"
      >
        <template #content>
          <template v-for="(seg, j) in segmentReferences(sent.text)" :key="j">
            <UBadge
              v-if="seg.kind === 'element'"
              color="neutral"
              variant="outline"
              size="sm"
              :icon="seg.icon"
              :label="seg.label"
              data-testid="element-tag"
              :title="seg.full"
              class="bg-default/60 align-baseline font-mono"
            />
            <template v-else>{{ seg.text }}</template>
          </template>
        </template>
      </UChatMessage>
    </template>
  </div>

  <UChatMessage
    v-else
    :id="message.id"
    role="assistant"
    :parts="[]"
    side="left"
    variant="naked"
    compact
    :ui="{ container: 'pb-0', content: 'flex flex-col gap-2.5 space-y-0 text-sm' }"
  >
    <template #content>
      <!-- Layout wrapper only: catches clicks on web links in the markdown. -->
      <div class="contents" @click.capture="openWebLink">
        <template v-for="(group, i) in groups" :key="i">
          <template v-if="group.kind === 'steps'">
            <UChatTool
              v-if="showStepsGroup(group)"
              data-testid="chat-steps"
              :data-status="group.working ? 'running' : undefined"
              :text="stepsTitle(group)"
              icon="i-hugeicons-check-list"
              :loading="group.working"
              :streaming="group.working"
              unmount-on-hide
              :ui="{ body: 'flex flex-col gap-1.5 whitespace-normal' }"
            >
              <template v-for="(step, j) in group.steps" :key="j">
                <UChatReasoning
                  v-if="step.type === 'reasoning'"
                  data-testid="chat-step"
                  :text="step.text"
                  :streaming="step.state === 'streaming'"
                  :auto-open="false"
                  icon="i-hugeicons-ai-brain-01"
                  unmount-on-hide
                />
                <UChatTool
                  v-else
                  data-testid="chat-step"
                  :data-status="toolStatus(step)"
                  :text="toolLabel(step)"
                  :icon="toolIcons[step.toolName] ?? 'i-hugeicons-wrench-01'"
                  :loading="toolStatus(step) === 'running'"
                  unmount-on-hide
                  :ui="{ body: 'flex flex-col gap-2 whitespace-normal' }"
                >
                  <!-- Read-only tool payloads; Nuxt UI's ProsePre is not generated (prose: false). -->
                  <pre data-testid="tool-code" :class="codeClass">{{ formatValue(toolArgs(step)) }}</pre>
                  <pre
                    v-if="step.state === 'output-available' && step.output != null"
                    data-testid="tool-code"
                    :class="codeClass"
                  >{{ formatValue(step.output) }}</pre>
                  <pre
                    v-if="step.state === 'output-error'"
                    data-testid="tool-error"
                    :class="[codeClass, 'border-error/25 bg-error/10 text-error']"
                  >{{ step.errorText }}</pre>
                </UChatTool>
              </template>
            </UChatTool>
          </template>

          <Markdown
            v-else-if="group.part.type === 'text'"
            :content="group.part.text"
            :parser-options="parserOptions"
          />

          <ChatCheckpoint
            v-else-if="group.part.type === 'data-checkpoint'"
            :project-id="projectId"
            :checkpoint="checkpoint(group.part.data)"
          />

          <template v-else-if="group.part.type === 'data-sync-status'">
            <UAlert
              v-if="syncIsTerminal(syncPhaseOf(group.part.data))"
              role="alert"
              color="neutral"
              variant="outline"
              :icon="syncPhaseOf(group.part.data) === 'combined' ? 'i-hugeicons-checkmark-circle-02' : 'i-hugeicons-information-circle'"
              :description="syncPhaseData(group.part.data).text"
              data-testid="sync-status"
            />
            <p v-else data-testid="sync-status" class="text-muted text-xs">
              {{ syncPhaseData(group.part.data).text }}
            </p>
          </template>

          <UAlert
            v-else-if="group.part.type === 'data-agent-error'"
            role="alert"
            color="error"
            variant="subtle"
            icon="i-hugeicons-alert-circle"
            data-testid="assistant-error"
          >
            <template #description>
              <div class="flex flex-col items-start gap-2">
                <span data-testid="assistant-error-message">{{ ERROR_TITLE[agentError(group.part.data).code] }}</span>
                <UButton
                  v-if="agentError(group.part.data).code === 'failed'"
                  color="neutral"
                  variant="outline"
                  size="sm"
                  label="Try again"
                  data-testid="assistant-error-retry"
                  @click="act('retry', group.part.data)"
                />
                <UButton
                  v-else-if="agentError(group.part.data).code === 'login'"
                  color="neutral"
                  variant="outline"
                  size="sm"
                  label="Sign in"
                  data-testid="assistant-error-sign-in"
                  @click="act('signIn', group.part.data)"
                />
                <UButton
                  v-else
                  color="neutral"
                  variant="outline"
                  size="sm"
                  label="Set up"
                  data-testid="assistant-error-setup"
                  @click="act('setup', group.part.data)"
                />
                <UCollapsible
                  v-if="agentError(group.part.data).code === 'failed'"
                  :unmount-on-hide="false"
                  data-testid="assistant-error-details"
                  class="text-xs"
                >
                  <UButton
                    color="neutral"
                    variant="link"
                    size="xs"
                    trailing-icon="i-hugeicons-arrow-down-01"
                    label="Details"
                    class="p-0"
                  />
                  <template #content>
                    <p class="mt-1 break-words opacity-90">{{ agentError(group.part.data).message }}</p>
                  </template>
                </UCollapsible>
              </div>
            </template>
          </UAlert>
        </template>
      </div>
    </template>
  </UChatMessage>
</template>
