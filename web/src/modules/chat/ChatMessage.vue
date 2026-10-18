<script setup lang="ts">
import { computed, reactive, type Component } from "vue";
import type { DynamicToolUIPart, UIMessage } from "ai";
import { Markdown, type StreamMarkdownParserOptions } from "vue-stream-markdown";
import "vue-stream-markdown/index.css";
import {
  Brain,
  CircleAlert,
  FilePen,
  FileText,
  Globe,
  MousePointerClick,
  Paperclip,
  Search,
  SquareTerminal,
  Wrench,
} from "@lucide/vue";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Bubble, BubbleContent } from "@/components/ui/bubble";
import { CodeBlock, CodeBlockContent } from "@/components/ui/code-block";
import { Marker, MarkerContent, MarkerIcon } from "@/components/ui/marker";
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
import ChatCheckpoint from "./ChatCheckpoint.vue";
import { splitAttachments } from "./attachments";
import { openWebLink } from "./links";
import { formatValue, toolLabel, type AgentErrorData, type CheckpointData, type ToolInput } from "./parts";
import { segmentReferences } from "./references";

const props = defineProps<{ projectId: string; message: UIMessage }>();
const daemon = useDaemon();

/** The user's text parts, with the attachment paths ChatPanel appended split back out. */
const userTexts = computed(() =>
  props.message.parts.flatMap((p) => (p.type === "text" ? [splitAttachments(p.text)] : [])),
);
/** Attached images that failed to load (e.g. a cross-site daemon); shown as file chips. */
const broken = reactive(new Set<string>());

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
</script>

<template>
  <Message v-if="message.role === 'user'" align="end">
    <MessageContent>
      <Bubble variant="secondary" align="end">
        <template v-for="(sent, i) in userTexts" :key="i">
          <div v-if="sent.files.length" class="flex flex-wrap justify-end gap-1.5">
            <template v-for="f in sent.files" :key="f.name">
              <a
                v-if="f.image && !broken.has(f.name)"
                data-testid="message-attachment"
                :href="daemon.api.attachmentUrl(projectId, f.name)"
                target="_blank"
                rel="noopener noreferrer"
                class="block overflow-hidden rounded-lg border"
              >
                <img
                  :src="daemon.api.attachmentUrl(projectId, f.name)"
                  :alt="`Attached ${f.kind} image`"
                  class="max-h-48 max-w-full object-contain"
                  @error="broken.add(f.name)"
                />
              </a>
              <span
                v-else
                data-testid="message-attachment"
                class="bg-secondary inline-flex items-center gap-1.5 rounded-md border px-2 py-1 text-xs"
              ><Paperclip class="size-3" aria-hidden="true" />{{ f.kind }} file</span>
            </template>
          </div>
          <BubbleContent v-if="sent.text" class="whitespace-pre-wrap">
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
        </template>
      </Bubble>
    </MessageContent>
  </Message>

  <Message v-else>
    <MessageContent @click.capture="openWebLink">
      <template v-for="(part, i) in message.parts" :key="i">
        <Markdown v-if="part.type === 'text'" :content="part.text" :parser-options="parserOptions" />

        <Trace v-else-if="part.type === 'reasoning'">
          <TraceCollapsible>
            <TraceCollapsibleTrigger>
              <TraceLine>
                <TraceIcon><Brain /></TraceIcon>
                <TraceTitle>Thinking</TraceTitle>
              </TraceLine>
            </TraceCollapsibleTrigger>
            <TraceCollapsibleContent class="whitespace-pre-wrap">{{ part.text }}</TraceCollapsibleContent>
          </TraceCollapsible>
        </Trace>

        <Trace v-else-if="part.type === 'dynamic-tool'">
          <TraceCollapsible>
            <TraceCollapsibleTrigger>
              <TraceLine :status="toolStatus(part)">
                <TraceIcon><component :is="toolIcons[part.toolName] ?? Wrench" /></TraceIcon>
                <TraceTitle>{{ toolLabel(part) }}</TraceTitle>
              </TraceLine>
            </TraceCollapsibleTrigger>
            <TraceCollapsibleContent class="flex flex-col gap-2">
              <CodeBlock size="sm">
                <CodeBlockContent wrap>{{ formatValue(toolArgs(part)) }}</CodeBlockContent>
              </CodeBlock>
              <CodeBlock v-if="part.state === 'output-available' && part.output != null" size="sm">
                <CodeBlockContent wrap>{{ formatValue(part.output) }}</CodeBlockContent>
              </CodeBlock>
              <CodeBlock v-if="part.state === 'output-error'" size="sm" variant="destructive" data-testid="tool-error">
                <CodeBlockContent wrap>{{ part.errorText }}</CodeBlockContent>
              </CodeBlock>
            </TraceCollapsibleContent>
          </TraceCollapsible>
        </Trace>

        <ChatCheckpoint
          v-else-if="part.type === 'data-checkpoint'"
          :project-id="projectId"
          :checkpoint="checkpoint(part.data)"
        />

        <Marker v-else-if="part.type === 'data-terminal'" variant="separator">
          <MarkerIcon><SquareTerminal /></MarkerIcon>
          <MarkerContent>Continued in Terminal</MarkerContent>
        </Marker>

        <Alert v-else-if="part.type === 'data-agent-error'" variant="destructive">
          <CircleAlert />
          <AlertDescription>{{ agentError(part.data).message }}</AlertDescription>
        </Alert>
      </template>
    </MessageContent>
  </Message>
</template>
