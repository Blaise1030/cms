import type { DynamicToolUIPart, ReasoningUIPart, UIMessage } from "ai";
import type { SyncPhase } from "@/core/types";

/** One stage line a sync chat shows; the daemon writes `text` for the writer. */
export interface SyncStatusData {
  phase: SyncPhase;
  text: string;
}

/** The only sync-chat wording the frontend owns; everything else comes from `sync_status`. */
export const PHASE_LABEL: Record<SyncPhase, string> = {
  working: "Combining…",
  checking: "Checking the site still works…",
  combined: "Combined",
  ended: "Ended",
  expired: "Ended",
};

export const SYNC_ACTIONS = {
  end: "End sync",
  pick: "Pick the changes myself",
  again: "Combine again",
} as const;

/** A combine that has landed or stopped: its chat shows the final actions, not a composer. */
export const syncIsTerminal = (phase: SyncPhase | null | undefined): boolean =>
  phase === "combined" || phase === "ended" || phase === "expired";

/** The phase badge's tone: a landed combine is the one that stands out. */
export const syncPhaseBadge = (phase: SyncPhase): "default" | "secondary" | "outline" =>
  phase === "combined" ? "default" : syncIsTerminal(phase) ? "outline" : "secondary";

/**
 * An outcome line ends the combine; a stage note is progress inside one. The
 * transcript calls out the former and leaves the latter as a quiet line — it
 * shares `syncIsTerminal` so both surfaces agree on what "finished" means.
 */
export const syncPhaseData = (data: unknown) => data as SyncStatusData;
export const syncPhaseOf = (data: unknown) => syncPhaseData(data).phase;

/** The daemon's normalized tool input: `raw` is the agent's own arguments. */
export interface ToolInput {
  path?: string;
  command?: string;
  query?: string;
  raw?: unknown;
}

export interface CheckpointData {
  sha?: string;
  summary?: string;
  files: string[];
  blocked?: string;
}

export interface AgentErrorData {
  code: "login" | "not_installed" | "failed";
  message: string;
}

const basename = (path = "") => path.split("/").pop() || path;

/** A raw name is only known when `raw` is the name itself, or names it (Codex MCP calls). */
function rawName(raw: unknown): string | undefined {
  if (typeof raw === "string") return raw;
  const tool = (raw as { tool?: unknown } | null | undefined)?.tool;
  return typeof tool === "string" ? tool : undefined;
}

export function toolLabel(part: DynamicToolUIPart): string {
  const input = (part.input ?? {}) as ToolInput;
  switch (part.toolName) {
    case "read":
      return `Read ${basename(input.path)}`;
    case "edit":
      return `Edited ${basename(input.path)}`;
    case "shell":
      return `Ran ${(input.command ?? "").split("\n")[0]!.slice(0, 60)}`;
    case "search":
      return `Searched ${input.query ?? ""}`;
    case "web":
      return "Searched the web";
    default:
      return `Used ${rawName(input.raw) ?? "a tool"}`;
  }
}

type Part = UIMessage["parts"][number];
export type StepPart = ReasoningUIPart | DynamicToolUIPart;
export type MessageGroup =
  | { kind: "steps"; steps: StepPart[]; working: boolean }
  | { kind: "part"; part: Part };

const isStep = (part: Part): part is StepPart => part.type === "reasoning" || part.type === "dynamic-tool";
const stepWorking = (part: StepPart) =>
  part.type === "reasoning"
    ? part.state === "streaming"
    : part.state !== "output-available" && part.state !== "output-error";

/** Each run of reasoning and tool parts becomes one steps group; `step-start` markers render nothing. */
export function groupSteps(parts: Part[]): MessageGroup[] {
  const groups: MessageGroup[] = [];
  for (const part of parts) {
    if (part.type === "step-start") continue;
    if (!isStep(part)) {
      groups.push({ kind: "part", part });
      continue;
    }
    const last = groups.at(-1);
    if (last?.kind === "steps") {
      last.steps.push(part);
      last.working ||= stepWorking(part);
    } else {
      groups.push({ kind: "steps", steps: [part], working: stepWorking(part) });
    }
  }
  return groups;
}

export const stepsTitle = (group: Extract<MessageGroup, { kind: "steps" }>) =>
  group.working
    ? "Working…"
    : `Worked through ${group.steps.length} ${group.steps.length === 1 ? "step" : "steps"}`;

/** Strings (diffs, truncated JSON text) show as-is; anything else as pretty JSON. */
export const formatValue = (value: unknown): string =>
  typeof value === "string" ? value : JSON.stringify(value, null, 2);
