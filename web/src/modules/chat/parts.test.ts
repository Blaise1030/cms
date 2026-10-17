import { describe, expect, it } from "vitest";
import type { DynamicToolUIPart } from "ai";
import { toolLabel } from "./parts";

const tool = (toolName: string, input: Record<string, unknown>) =>
  ({ type: "dynamic-tool", toolName, toolCallId: "t1", state: "input-available", input }) as DynamicToolUIPart;

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
