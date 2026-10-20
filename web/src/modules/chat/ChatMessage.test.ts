// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import type { UIMessage } from "ai";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("vue-sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

import ChatMessage from "./ChatMessage.vue";

let api: Record<string, ReturnType<typeof vi.fn>>;

async function render(parts: unknown[], role: UIMessage["role"] = "assistant") {
  api = { restoreCheckpoint: vi.fn(async () => ({ restored: "abc1234def" })) };
  h.daemon = createFakeDaemon(api);
  const message = { id: "m1", role, parts } as UIMessage;
  const w = mount(ChatMessage, { props: { projectId: "p", message }, attachTo: document.body });
  await flushPromises();
  return w;
}

const byText = (text: string) =>
  [...document.body.querySelectorAll("button")].find((b) => b.textContent?.includes(text));

/** Expands the collapsed steps group, then the step inside it at `index`. */
async function openStep(w: Awaited<ReturnType<typeof render>>, index = 0) {
  const group = w.find("[data-testid='chat-steps'] [data-slot='trace-collapsible-trigger']");
  if (group.attributes("aria-expanded") !== "true") await group.trigger("click");
  await flushPromises();
  await w.findAll("[data-testid='chat-steps'] [data-slot='trace-collapsible-content'] [data-slot='trace-collapsible-trigger']")[index]!.trigger("click");
  await flushPromises();
}

afterEach(() => {
  document.body.innerHTML = "";
});

describe("ChatMessage", () => {
  it("renders text as markdown", async () => {
    const w = await render([{ type: "text", text: "Hello **b**" }]);
    expect(w.find("strong").text()).toBe("b");
    w.unmount();
  });

  it("does not render raw HTML from the agent", async () => {
    const w = await render([
      { type: "text", text: 'See <iframe src="https://x"></iframe>\n\n<form><input name="q"></form>\n\n**ok**' },
    ]);
    expect(w.find("strong").text()).toBe("ok");
    expect(document.body.querySelector("iframe")).toBeNull();
    expect(document.body.querySelector("form")).toBeNull();
    expect(document.body.querySelector("input")).toBeNull();
    w.unmount();
  });

  it("shows attached images as previews and other files as chips instead of their paths", async () => {
    const dir = "/Users/me/orso-projects/.orso/p/attachments";
    const w = await render(
      [{ type: "text", text: `Test\n${dir}/5d83a351365d7d18.png\n${dir}/0123456789abcdef.pdf` }],
      "user",
    );
    const img = w.find<HTMLImageElement>("[data-testid='message-attachment'] img");
    expect(img.attributes("src")).toBe("http://127.0.0.1:7800/api/terminal/p/attachments/5d83a351365d7d18.png");
    const files = w.findAll("[data-testid='message-attachment']");
    expect(files).toHaveLength(2);
    expect(files[1]!.text()).toContain("PDF file");
    expect(w.text()).toContain("Test");
    expect(w.text()).not.toContain("attachments/");
    w.unmount();
  });

  it("falls back to a chip when an attached image can't load", async () => {
    const w = await render(
      [{ type: "text", text: "/s/.orso/p/attachments/5d83a351365d7d18.png" }],
      "user",
    );
    await w.find("[data-testid='message-attachment'] img").trigger("error");
    expect(w.find("[data-testid='message-attachment'] img").exists()).toBe(false);
    expect(w.find("[data-testid='message-attachment']").text()).toContain("PNG file");
    w.unmount();
  });

  it("shows a picked element in a user message as a compact tag", async () => {
    const ref = '[element <section class="mx-auto" id="hero"> "Stop building" in src/Hero.astro:5:69]';
    const shot = "/p/.orso/site/attachments/4b78.png";
    const w = await render([{ type: "text", text: `${ref} ${shot} make it shorter` }], "user");
    const tag = w.find("[data-testid='element-tag']");
    expect(tag.text()).toBe("section#hero");
    expect(tag.attributes("title")).toBe(`${ref} ${shot}`);
    expect(w.text()).toContain("make it shorter");
    expect(w.text()).not.toContain("attachments");
    expect(w.text()).not.toContain("[element");
    w.unmount();
  });

  it("renders a user message's text", async () => {
    const w = await render([{ type: "text", text: "Make the hero bigger" }], "user");
    expect(w.text()).toContain("Make the hero bigger");
    w.unmount();
  });

  it("opens a website link in the default browser instead of navigating the app", async () => {
    const open = vi.fn();
    window.open = open;
    const w = await render([{ type: "text", text: "See [the docs](https://example.com/docs) and [local](/preview/p1/)" }]);
    const links = [...document.body.querySelectorAll("a")];
    const external = links.find((a) => a.getAttribute("href") === "https://example.com/docs")!;
    expect(external).toBeDefined();
    const notPrevented = external.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    expect(notPrevented).toBe(false);
    expect(open).toHaveBeenCalledWith("https://example.com/docs", "_blank", "noopener,noreferrer");
    w.unmount();
  });

  it("collapses a run of steps into one row, keeping the agent's reply outside it", async () => {
    const w = await render([
      { type: "step-start" },
      { type: "reasoning", text: "Pondering", state: "done" },
      { type: "dynamic-tool", toolName: "shell", toolCallId: "t1", state: "output-available", input: { command: "ls" }, output: "" },
      { type: "dynamic-tool", toolName: "read", toolCallId: "t2", state: "output-error", input: { path: "/a.md" }, errorText: "nope" },
      { type: "text", text: "Fixed it" },
    ]);
    const steps = w.findAll("[data-testid='chat-steps']");
    expect(steps).toHaveLength(1);
    expect(steps[0]!.text()).toContain("Worked through 3 steps");
    expect(w.text()).not.toContain("Thinking");
    expect(w.text()).not.toContain("Ran ls");
    expect(w.text()).toContain("Fixed it");
    await w.find("[data-testid='chat-steps'] [data-slot='trace-collapsible-trigger']").trigger("click");
    await flushPromises();
    expect(w.text()).toContain("Thinking");
    expect(w.text()).toContain("Ran ls");
    expect(w.text()).toContain("Read a.md");
    w.unmount();
  });

  it("shows a running group as working", async () => {
    const w = await render([
      { type: "dynamic-tool", toolName: "shell", toolCallId: "t1", state: "input-available", input: { command: "ls" } },
    ]);
    expect(w.find("[data-testid='chat-steps']").text()).toContain("Working…");
    expect(w.find("[data-testid='chat-steps'] [data-slot='trace-line']").attributes("data-status")).toBe("running");
    w.unmount();
  });

  it("shows reasoning behind a collapsed Thinking trigger", async () => {
    const w = await render([{ type: "step-start" }, { type: "reasoning", text: "Pondering the layout" }]);
    expect(w.text()).not.toContain("Pondering the layout");
    await openStep(w);
    expect(w.text()).toContain("Thinking");
    expect(w.text()).toContain("Pondering the layout");
    w.unmount();
  });

  it("shows a tool's label, and its input and output when expanded", async () => {
    const w = await render([
      {
        type: "dynamic-tool",
        toolName: "shell",
        toolCallId: "t1",
        state: "output-available",
        input: { command: "pnpm build", raw: { command: "pnpm build" } },
        output: { exitCode: 0, stdout: "built in 2s" },
      },
    ]);
    await w.find("[data-testid='chat-steps'] [data-slot='trace-collapsible-trigger']").trigger("click");
    await flushPromises();
    expect(w.text()).toContain("Ran pnpm build");
    expect(w.find("[data-slot='code-block']").exists()).toBe(false);
    await openStep(w);
    const blocks = w.findAll("[data-slot='code-block']");
    expect(blocks.map((b) => b.text()).join("\n")).toContain('"command": "pnpm build"');
    expect(blocks.map((b) => b.text()).join("\n")).toContain('"stdout": "built in 2s"');
    w.unmount();
  });

  it("renders string output as-is", async () => {
    const w = await render([
      {
        type: "dynamic-tool",
        toolName: "edit",
        toolCallId: "t1",
        state: "output-available",
        input: { path: "/work/project/src/about.md", raw: {} },
        output: "-old\n+new",
      },
    ]);
    await openStep(w);
    expect(w.text()).toContain("Edited about.md");
    expect(w.text()).toContain("-old");
    expect(w.text()).not.toContain('"-old');
    w.unmount();
  });

  it("shows a tool's error text", async () => {
    const w = await render([
      {
        type: "dynamic-tool",
        toolName: "read",
        toolCallId: "t1",
        state: "output-error",
        input: { path: "/work/project/missing.md", raw: {} },
        errorText: "No such file",
      },
    ]);
    await openStep(w);
    expect(w.find("[data-testid='tool-error']").text()).toContain("No such file");
    w.unmount();
  });

  it("counts saved files in a checkpoint", async () => {
    const one = await render([{ type: "data-checkpoint", data: { sha: "abc1234def", files: ["a.md"] } }]);
    expect(one.text()).toContain("Saved 1 file");
    one.unmount();
    const two = await render([{ type: "data-checkpoint", data: { sha: "abc1234def", files: ["a.md", "b.md"] } }]);
    expect(two.text()).toContain("Saved 2 files");
    expect(byText("Restore")).toBeDefined();
    two.unmount();
  });

  it("restores a checkpoint only after confirming", async () => {
    const w = await render([
      { type: "data-checkpoint", data: { sha: "abc1234def", summary: "hero", files: ["a.md", "b.md"] } },
    ]);
    byText("Restore")!.click();
    await flushPromises();
    expect(document.body.textContent).toContain("Restore this checkpoint?");
    expect(api.restoreCheckpoint).not.toHaveBeenCalled();
    byText("Cancel")!.click();
    await flushPromises();
    expect(api.restoreCheckpoint).not.toHaveBeenCalled();

    byText("Restore")!.click();
    await flushPromises();
    (document.body.querySelector("[data-testid='confirm-restore']") as HTMLElement).click();
    await flushPromises();
    expect(api.restoreCheckpoint).toHaveBeenCalledWith("p", "abc1234def");
    w.unmount();
  });

  it("explains a blocked checkpoint", async () => {
    const w = await render([{ type: "data-checkpoint", data: { files: [], blocked: "merge in progress" } }]);
    expect(w.text()).toContain("Not saved: merge in progress");
    expect(byText("Restore")).toBeUndefined();
    w.unmount();
  });

  it("marks a turn continued in the terminal", async () => {
    const w = await render([{ type: "data-terminal", data: {} }]);
    expect(w.text()).toContain("Continued in Terminal");
    w.unmount();
  });

  it.each([
    ["login", "Log in to Claude"],
    ["not_installed", "Claude isn't installed"],
    ["failed", "exit 1"],
  ])("shows only the message for a %s agent error", async (code, message) => {
    const w = await render([{ type: "data-agent-error", data: { code, message } }]);
    expect(w.find("[role='alert']").text()).toBe(message);
    expect(w.find("button").exists()).toBe(false);
    w.unmount();
  });
});
