<script setup lang="ts">
import type { Component } from "vue";
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
  Search,
  SquareTerminal,
  Wrench,
} from "@lucide/vue";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Bubble, BubbleContent } from "@/components/ui/bubble";
import { Button } from "@/components/ui/button";
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
import ChatCheckpoint from "./ChatCheckpoint.vue";
import { formatValue, toolLabel, type AgentErrorData, type CheckpointData, type ToolInput } from "./parts";
import { segmentReferences } from "./references";

defineProps<{ projectId: string; message: UIMessage }>();
defineEmits<{ "open-terminal": [] }>();

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
        <template v-for="(part, i) in message.parts" :key="i">
          <BubbleContent v-if="part.type === 'text'" class="whitespace-pre-wrap">
            <template v-for="(seg, j) in segmentReferences(part.text)" :key="j">
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
    <MessageContent>
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
          <AlertDescription class="flex flex-wrap items-center gap-2">
            <span class="min-w-0 flex-1">{{ agentError(part.data).message }}</span>
            <Button
              v-if="agentError(part.data).code !== 'failed'"
              variant="outline"
              size="sm"
              @click="$emit('open-terminal')"
            >
              Open Terminal
            </Button>
          </AlertDescription>
        </Alert>
      </template>
    </MessageContent>
  </Message>
</template>
