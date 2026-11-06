<script setup lang="ts">
import { computed, reactive, type Component } from "vue";
import type { DynamicToolUIPart, UIMessage } from "ai";
import { Markdown, type StreamMarkdownParserOptions } from "vue-stream-markdown";
import "vue-stream-markdown/index.css";
import {
  Brain,
  CircleAlert,
  CircleCheck,
  FilePen,
  FileText,
  Globe,
  Info,
  ListChecks,
  MousePointerClick,
  Paperclip,
  Search,
  SquareTerminal,
  Wrench,
} from "@lucide/vue";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Bubble, BubbleContent, BubbleGroup } from "@/components/ui/bubble";
import { Button } from "@/components/ui/button";
import { CodeBlock, CodeBlockContent } from "@/components/ui/code-block";
import { Message, MessageContent } from "@/components/ui/message";
import {
  Trace,
  TraceCollapsible,
  TraceCollapsibleContent,
  TraceCollapsibleTrigger,
  TraceIcon,
  TraceLine,
  TraceTitle,
} from "@/components/ui/trace";
import { useDaemon } from "@/core/daemon";
import { useChatSteps } from "@/lib/chatSteps";
import ChatCheckpoint from "./ChatCheckpoint.vue";
import { splitAttachments } from "./attachments";
import { openWebLink } from "./links";
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

/** The user's text parts, with the attachment paths ChatPanel appended split back out. */
const userTexts = computed(() =>
  props.message.parts.flatMap((p) => (p.type === "text" ? [splitAttachments(p.text)] : [])),
);
const groups = computed(() => groupSteps(props.message.parts));
/** Attached images that failed to load (e.g. a cross-site daemon); shown as file chips. */
const broken = reactive(new Set<string>());

/** Finished steps follow Appearance; in-flight “Working…” always stays visible. */
const showStepsGroup = (group: Extract<MessageGroup, { kind: "steps" }>) =>
  group.working || chatSteps.enabled.value;

/** Agent text is untrusted: replacing the parser's default `html` plugin leaves raw HTML as escaped text. */
const parserOptions: StreamMarkdownParserOptions = { plugins: [{ name: "html" }] };

const toolIcons: Record<string, Component> = {
  read: FileText,
  edit: FilePen,
  shell: SquareTerminal,
  search: Search,
  web: Globe,
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

function act(action: "setup" | "signIn" | "retry", data: unknown) {
  emit("assistantAction", { action, provider: agentError(data).provider ?? props.provider ?? "" });
}
</script>

<template>
  <!-- Padding keeps btn-raised-soft's shadow inside the scroller item (content-visibility clips paint). -->
  <Message v-if="message.role === 'user'" align="end" class="p-1">
    <MessageContent>
      <BubbleGroup class="items-end">
        <template v-for="(sent, i) in userTexts" :key="i">
          <template v-for="f in sent.files" :key="f.name">
            <Bubble
              v-if="f.image && !broken.has(f.name)"
              variant="tinted"
              align="end"
              class="overflow-hidden p-1"
            >
              <a
                data-testid="message-attachment"
                :href="daemon.api.attachmentUrl(projectId, f.name)"
                target="_blank"
                rel="noopener noreferrer"
                class="block overflow-hidden rounded-lg"
              >
                <img
                  :src="daemon.api.attachmentUrl(projectId, f.name)"
                  :alt="`Attached ${f.kind} image`"
                  class="max-h-48 max-w-full object-contain"
                  @error="broken.add(f.name)"
                />
              </a>
            </Bubble>
          </template>
          <Bubble
            v-if="sent.files.some((f) => !f.image || broken.has(f.name))"
            variant="tinted"
            align="end"
          >
            <BubbleContent class="flex flex-wrap justify-end gap-1.5">
              <template v-for="f in sent.files" :key="f.name">
                <span
                  v-if="!f.image || broken.has(f.name)"
                  data-testid="message-attachment"
                  class="bg-secondary inline-flex items-center gap-1.5 rounded-md border px-2 py-1 text-xs"
                ><Paperclip class="size-3" aria-hidden="true" />{{ f.kind }} file</span>
              </template>
            </BubbleContent>
          </Bubble>
          <Bubble v-if="sent.text" variant="tinted" align="end">
            <BubbleContent class="whitespace-pre-wrap">
              <template v-for="(seg, j) in segmentReferences(sent.text)" :key="j">
                <span
                  v-if="seg.kind === 'element'"
                  data-testid="element-tag"
                  :title="seg.full"
                  class="bg-background/60 inline-flex items-center gap-1 rounded-md border px-1.5 align-baseline font-mono text-xs"
                ><MousePointerClick class="size-3" aria-hidden="true" />{{ seg.label }}</span>
                <template v-else>{{ seg.text }}</template>
              </template>
            </BubbleContent>
          </Bubble>
        </template>
      </BubbleGroup>
    </MessageContent>
  </Message>

  <Message v-else>
    <MessageContent @click.capture="openWebLink">
      <template v-for="(group, i) in groups" :key="i">
        <template v-if="group.kind === 'steps'">
          <Trace v-if="showStepsGroup(group)" data-testid="chat-steps">
            <TraceCollapsible>
              <TraceCollapsibleTrigger>
                <TraceLine :status="group.working ? 'running' : undefined">
                  <TraceIcon><ListChecks /></TraceIcon>
                  <TraceTitle>{{ stepsTitle(group) }}</TraceTitle>
                </TraceLine>
              </TraceCollapsibleTrigger>
              <TraceCollapsibleContent>
                <Trace>
                  <TraceCollapsible v-for="(step, j) in group.steps" :key="j">
                    <template v-if="step.type === 'reasoning'">
                      <TraceCollapsibleTrigger>
                        <TraceLine>
                          <TraceIcon><Brain /></TraceIcon>
                          <TraceTitle>Thinking</TraceTitle>
                        </TraceLine>
                      </TraceCollapsibleTrigger>
                      <TraceCollapsibleContent class="whitespace-pre-wrap">{{ step.text }}</TraceCollapsibleContent>
                    </template>
                    <template v-else>
                      <TraceCollapsibleTrigger>
                        <TraceLine :status="toolStatus(step)">
                          <TraceIcon><component :is="toolIcons[step.toolName] ?? Wrench" /></TraceIcon>
                          <TraceTitle>{{ toolLabel(step) }}</TraceTitle>
                        </TraceLine>
                      </TraceCollapsibleTrigger>
                      <TraceCollapsibleContent class="flex flex-col gap-2">
                        <CodeBlock size="sm">
                          <CodeBlockContent wrap>{{ formatValue(toolArgs(step)) }}</CodeBlockContent>
                        </CodeBlock>
                        <CodeBlock v-if="step.state === 'output-available' && step.output != null" size="sm">
                          <CodeBlockContent wrap>{{ formatValue(step.output) }}</CodeBlockContent>
                        </CodeBlock>
                        <CodeBlock v-if="step.state === 'output-error'" size="sm" variant="destructive" data-testid="tool-error">
                          <CodeBlockContent wrap>{{ step.errorText }}</CodeBlockContent>
                        </CodeBlock>
                      </TraceCollapsibleContent>
                    </template>
                  </TraceCollapsible>
                </Trace>
              </TraceCollapsibleContent>
            </TraceCollapsible>
          </Trace>
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
          <Alert
            v-if="syncIsTerminal(syncPhaseOf(group.part.data))"
            data-testid="sync-status"
          >
            <CircleCheck v-if="syncPhaseOf(group.part.data) === 'combined'" />
            <Info v-else />
            <AlertDescription>{{ syncPhaseData(group.part.data).text }}</AlertDescription>
          </Alert>
          <p v-else data-testid="sync-status" class="text-muted-foreground text-xs">
            {{ syncPhaseData(group.part.data).text }}
          </p>
        </template>

        <Alert v-else-if="group.part.type === 'data-agent-error'" variant="destructive" data-testid="assistant-error">
          <CircleAlert />
          <AlertDescription class="flex flex-col items-start gap-2">
            <span data-testid="assistant-error-message">{{ ERROR_TITLE[agentError(group.part.data).code] }}</span>
            <Button
              v-if="agentError(group.part.data).code === 'failed'"
              size="sm"
              variant="outline"
              data-testid="assistant-error-retry"
              @click="act('retry', group.part.data)"
            >
              Try again
            </Button>
            <Button
              v-else-if="agentError(group.part.data).code === 'login'"
              size="sm"
              variant="outline"
              data-testid="assistant-error-sign-in"
              @click="act('signIn', group.part.data)"
            >
              Sign in
            </Button>
            <Button
              v-else
              size="sm"
              variant="outline"
              data-testid="assistant-error-setup"
              @click="act('setup', group.part.data)"
            >
              Set up
            </Button>
            <details
              v-if="agentError(group.part.data).code === 'failed'"
              data-testid="assistant-error-details"
              class="text-xs"
            >
              <summary class="cursor-pointer">Details</summary>
              <p class="mt-1 break-words opacity-90">{{ agentError(group.part.data).message }}</p>
            </details>
          </AlertDescription>
        </Alert>
      </template>
    </MessageContent>
  </Message>
</template>
