import { describe, expect, it } from "vitest";
import type { DynamicToolUIPart, UIMessage } from "ai";
import { groupSteps, stepsTitle, toolLabel, type StepPart } from "./parts";

const tool = (toolName: string, input: Record<string, unknown>) =>
  ({ type: "dynamic-tool", toolName, toolCallId: "t1", state: "input-available", input }) as DynamicToolUIPart;

type Part = UIMessage["parts"][number];
const done = (toolName: string, state = "output-available") =>
  ({ type: "dynamic-tool", toolName, toolCallId: "t", state, input: {}, output: "" }) as StepPart;
const thought = (state?: "streaming" | "done") => ({ type: "reasoning", text: "hm", state }) as StepPart;
const text = (t: string) => ({ type: "text", text: t }) as Part;

describe("groupSteps", () => {
  it("collapses each run of reasoning and tool parts into one group, split by other parts", () => {
    const groups = groupSteps([
      { type: "step-start" },
      thought("done"),
      done("shell"),
      done("read", "output-error"),
      text("Fixed it"),
      done("edit"),
    ] as Part[]);
    expect(groups.map((g) => g.kind)).toEqual(["steps", "part", "steps"]);
    expect(groups[0]).toMatchObject({ kind: "steps", working: false });
    expect(groups[0]!.kind === "steps" && groups[0]!.steps).toHaveLength(3);
  });

  it("is working while a tool hasn't finished or reasoning is still streaming", () => {
    expect(groupSteps([done("shell"), tool("read", {})])[0]).toMatchObject({ working: true });
    expect(groupSteps([thought("streaming")])[0]).toMatchObject({ working: true });
  });

  it("titles a group by its step count, or as working", () => {
    expect(stepsTitle({ kind: "steps", steps: [thought()], working: false })).toBe("Worked through 1 step");
    expect(stepsTitle({ kind: "steps", steps: [thought(), done("shell")], working: false })).toBe(
      "Worked through 2 steps",
    );
    expect(stepsTitle({ kind: "steps", steps: [thought()], working: true })).toBe("Working…");
  });
});

describe("toolLabel", () => {
  it("names read and edit tools by basename", () => {
    expect(toolLabel(tool("read", { path: "/work/project/src/index.md" }))).toBe("Read index.md");
    expect(toolLabel(tool("edit", { path: "/work/project/src/about.md" }))).toBe("Edited about.md");
  });

  it("shows the first line of a shell command, capped at 60 chars", () => {
    expect(toolLabel(tool("shell", { command: "pnpm build\necho done" }))).toBe("Ran pnpm build");
    const long = "x".repeat(80);
    expect(toolLabel(tool("shell", { command: long }))).toBe(`Ran ${"x".repeat(60)}`);
  });

  it("names searches", () => {
    expect(toolLabel(tool("search", { query: "hero" }))).toBe("Searched hero");
    expect(toolLabel(tool("web", { query: "vue" }))).toBe("Searched the web");
  });

  it("uses the raw tool name for mcp and other tools", () => {
    expect(toolLabel(tool("mcp", { raw: { server: "figma", tool: "get_file", arguments: {} } }))).toBe(
      "Used get_file",
    );
    expect(toolLabel(tool("other", { raw: "TodoWrite" }))).toBe("Used TodoWrite");
    expect(toolLabel(tool("other", { raw: { todos: [] } }))).toBe("Used a tool");
    expect(toolLabel(tool("mcp", {}))).toBe("Used a tool");
  });
});
