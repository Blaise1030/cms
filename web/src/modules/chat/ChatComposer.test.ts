// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import ChatComposer from "./ChatComposer.vue";

enableAutoUnmount(afterEach);

/** The pickers are real Nuxt UI dropdowns (teleported to body); in tests the provider flyouts open with the menu. */
const stubs = {};

async function openPicker(w: { find: (s: string) => { trigger: (e: string, o?: object) => Promise<void> } }) {
  await w.find("[data-testid='chat-agent']").trigger("keydown", { key: "Enter" });
  await flushPromises();
}
const inBody = (selector: string) => [...document.body.querySelectorAll<HTMLElement>(selector)];

function mountComposer(autoApprove: boolean, locked: string | null = null) {
  return mount(ChatComposer, {
    props: {
      modelValue: "",
      provider: "cursor",
      model: null,
      providers: ["cursor"],
      models: {},
      providerLocked: false,
      streaming: false,
      blocked: null,
      locked,
      attachments: [],
      attaching: false,
      references: [],
      autoApprove,
    },
    global: { stubs },
    attachTo: document.body,
  });
}

describe("ChatComposer page context", () => {
  it("shows a badge for the current page context", () => {
    const w = mount(ChatComposer, {
      props: {
        modelValue: "",
        provider: "cursor",
        model: null,
        providers: ["cursor"],
        models: {},
        providerLocked: false,
        streaming: false,
        blocked: null,
        attachments: [],
        attaching: false,
        references: [],
        autoApprove: false,
        pageContext: { tab: "preview", fields: { path: "/about" } },
      },
      global: { stubs },
    });
    expect(w.find("[data-testid='chat-page-context']").text()).toContain("Preview · /about");
  });

  it("hides the badge when there is no page context", () => {
    expect(mountComposer(false).find("[data-testid='chat-page-context']").exists()).toBe(false);
  });

  it("offers a dismiss control before the context label", async () => {
    const w = mount(ChatComposer, {
      props: {
        modelValue: "",
        provider: "cursor",
        model: null,
        providers: ["cursor"],
        models: {},
        providerLocked: false,
        streaming: false,
        blocked: null,
        attachments: [],
        attaching: false,
        references: [],
        autoApprove: false,
        pageContext: { tab: "preview", fields: { path: "/about" } },
      },
      global: { stubs },
    });
    const badge = w.find("[data-testid='chat-page-context']");
    const remove = badge.find("[data-testid='chat-page-context-remove']");
    expect(remove.exists()).toBe(true);
    expect(badge.element.firstElementChild).toBe(remove.element);
    await remove.trigger("click");
    expect(w.emitted("clearPageContext")).toEqual([[]]);
  });
});

describe("ChatComposer lock", () => {
  it("shows the reason in place of the input", () => {
    const w = mountComposer(false, "Checking the site still works…");
    expect(w.find("[data-testid='chat-locked']").text()).toBe("Checking the site still works…");
    expect(w.find("[data-testid='chat-input']").exists()).toBe(false);
  });

  it("shows the input when nothing is locking it", () => {
    const w = mountComposer(false);
    expect(w.find("[data-testid='chat-input']").exists()).toBe(true);
    expect(w.find("[data-testid='chat-locked']").exists()).toBe(false);
  });
});

describe("ChatComposer approvals menu", () => {
  it("shows the current approval mode on the trigger", () => {
    expect(mountComposer(false).find("[data-testid='chat-auto-approve']").text()).toContain("Ask");
    expect(mountComposer(true).find("[data-testid='chat-auto-approve']").text()).toContain("Auto");
  });

  // The approvals menu is a real Nuxt UI dropdown (teleported to body), so open it first.
  async function openApprovals(autoApprove: boolean) {
    const w = mount(ChatComposer, {
      props: {
        modelValue: "",
        provider: "cursor",
        model: null,
        providers: ["cursor"],
        models: {},
        providerLocked: false,
        streaming: false,
        blocked: null,
        attachments: [],
        attaching: false,
        references: [],
        autoApprove,
      },
      global: { stubs },
      attachTo: document.body,
    });
    await w.find("[data-testid='chat-auto-approve']").trigger("keydown", { key: "Enter" });
    await flushPromises();
    const item = (id: string) => document.body.querySelector<HTMLElement>(`[data-testid='${id}']`);
    return { w, item };
  }

  it("offers both modes and emits the picked one", async () => {
    const { w, item } = await openApprovals(false);
    expect(item("chat-approval-ask")?.textContent).toContain("Ask each time");
    expect(item("chat-approval-auto")?.textContent).toContain("Auto-approve");
    item("chat-approval-auto")!.click();
    await flushPromises();
    await w.find("[data-testid='chat-auto-approve']").trigger("keydown", { key: "Enter" });
    await flushPromises();
    item("chat-approval-ask")!.click();
    await flushPromises();
    expect(w.emitted("setAutoApprove")).toEqual([[true], [false]]);
  });

  it("marks the mode in force", async () => {
    const { item } = await openApprovals(true);
    expect(item("chat-approval-auto")?.querySelector("svg")).not.toBeNull();
    expect(item("chat-approval-ask")?.querySelector("svg")).toBeNull();
  });
});

describe("ChatComposer assistant setup", () => {
  it("ends the assistant list with a setup action and emits it", async () => {
    const w = mountComposer(false);
    await openPicker(w);
    const item = inBody("[data-testid='chat-agent-setup']")[0]!;
    expect(item.textContent).toContain("Set up another assistant…");
    item.click();
    await flushPromises();
    expect(w.emitted("setup")).toHaveLength(1);
    expect(w.find("[data-testid='chat-agent']").attributes("data-provider")).toBe("cursor");
  });

  it("uses assistant wording, never developer jargon", () => {
    const w = mountComposer(false);
    const input = w.find<HTMLTextAreaElement>("[data-testid='chat-input']");
    expect(input.attributes("placeholder")).toBe("Ask your assistant to change your site");
    expect(w.find("[data-testid='chat-agent']").attributes("aria-label")).toBe("Assistant and model");
    const text = w.text().toLowerCase();
    for (const word of ["agent", "cli", "acp", "sdk", "npx", "path"]) {
      expect(text).not.toContain(word);
    }
  });
});

describe("ChatComposer model groups", () => {
  it("lists providers first; models and search live in the hover flyout", async () => {
    const w = mount(ChatComposer, {
      props: {
        modelValue: "",
        provider: "opencode",
        model: null,
        providers: ["opencode"],
        models: {
          opencode: [
            { id: "deepseek/deepseek-flash", name: "deepseek/deepseek-flash" },
            { id: "opencode-go/beam-free", name: "opencode-go/beam-free" },
          ],
        },
        providerLocked: false,
        streaming: false,
        blocked: null,
        attachments: [],
        attaching: false,
        references: [],
        autoApprove: false,
      },
      global: { stubs },
      attachTo: document.body,
    });
    await openPicker(w);

    expect(inBody("[data-testid='chat-agent-sub'][data-provider='opencode']")).toHaveLength(1);
    expect(inBody("input[data-testid='chat-model-search']")).toHaveLength(1);

    const subs = inBody("[data-testid='chat-model-sub']").map((el) =>
      el.getAttribute("data-model-provider"),
    );
    expect(subs).toEqual(["deepseek", "opencode-go"]);

    const topLabels = inBody("[data-testid='chat-model-item']").map((el) =>
      el.querySelector("[data-testid='chat-model-label']")?.textContent?.trim(),
    );
    expect(topLabels).toContain("Default");
    expect(topLabels).not.toContain("deepseek-flash");

    const nested = inBody("[data-testid='chat-model-sub-item']").map((el) =>
      el.querySelector("[data-testid='chat-model-label']")?.textContent?.trim(),
    );
    expect(nested).toEqual(["deepseek-flash", "beam-free"]);
  });

  it("shows every assistant as a provider flyout when several are available", async () => {
    const w = mount(ChatComposer, {
      props: {
        modelValue: "",
        provider: "cursor",
        model: null,
        providers: ["cursor", "claude", "opencode"],
        models: {
          cursor: [{ id: "cursor-smart", name: "Smart" }],
          claude: [{ id: "sonnet", name: "Sonnet" }],
          opencode: [{ id: "deepseek/flash", name: "deepseek/flash" }],
        },
        providerLocked: false,
        streaming: false,
        blocked: null,
        attachments: [],
        attaching: false,
        references: [],
        autoApprove: false,
      },
      global: { stubs },
      attachTo: document.body,
    });
    await openPicker(w);

    const agents = inBody("[data-testid='chat-agent-sub']").map((el) =>
      el.getAttribute("data-provider"),
    );
    expect(agents).toEqual(["cursor", "claude", "opencode"]);
    expect(inBody("[data-testid='chat-model-item'][data-value='cursor:default']")).toHaveLength(1);
    expect(inBody("[data-testid='chat-model-sub'][data-model-provider='deepseek']")).toHaveLength(1);
  });
});

describe("ChatComposer model picker", () => {
  const props = {
    modelValue: "",
    provider: "claude",
    model: "sonnet",
    providers: ["cursor", "claude"],
    models: { cursor: [{ id: "cursor-smart", name: "Smart" }], claude: [{ id: "sonnet", name: "Sonnet" }] },
    providerLocked: false,
    streaming: false,
    blocked: null,
    attachments: [],
    attaching: false,
    references: [],
    autoApprove: false,
  };

  it("checks the selected provider and model, and picks across providers", async () => {
    const w = mount(ChatComposer, { props, attachTo: document.body });
    await openPicker(w);
    const sub = (p: string) => inBody(`[data-testid='chat-agent-sub'][data-provider='${p}']`)[0]!;
    // The selected provider's row carries a check beside its flyout chevron.
    expect(sub("claude").closest("[data-slot='item']")!.querySelectorAll("[data-slot='itemTrailing'] svg, [data-slot='itemTrailing'] span.iconify").length).toBeGreaterThan(1);
    expect(sub("cursor").closest("[data-slot='item']")!.querySelectorAll("[data-slot='itemTrailing'] svg, [data-slot='itemTrailing'] span.iconify").length).toBe(1);
    const row = (value: string) => inBody(`[data-value='${value}']`)[0]!;
    const checked = (value: string) =>
      row(value).querySelector("[data-slot='itemTrailing']")!.children.length > 0;
    expect(checked("claude:sonnet")).toBe(true);
    expect(checked("claude:default")).toBe(false);
    expect(checked("cursor:cursor-smart")).toBe(false);
    row("cursor:cursor-smart").click();
    await flushPromises();
    expect(w.emitted("update:provider")).toEqual([["cursor"]]);
    expect(w.emitted("update:model")).toEqual([["cursor-smart"]]);
  });

  it("filters a flyout's models with its search box", async () => {
    const w = mount(ChatComposer, {
      props: {
        ...props,
        provider: "opencode",
        model: null,
        providers: ["opencode"],
        models: { opencode: [{ id: "deepseek/flash", name: "deepseek/flash" }, { id: "qwen/coder", name: "qwen/coder" }] },
      },
      attachTo: document.body,
    });
    await openPicker(w);
    const search = inBody("input[data-testid='chat-model-search']")[0] as HTMLInputElement;
    expect(search.placeholder).toBe("Search models…");
    search.value = "qwen";
    search.dispatchEvent(new Event("input"));
    await flushPromises();
    expect(inBody("[data-testid='chat-model-sub-item']").map((e) => e.dataset.value)).toEqual(["opencode:qwen/coder"]);
    expect(inBody("[data-testid='chat-model-sub']").map((e) => e.dataset.modelProvider)).toEqual(["qwen"]);
  });
});

describe("ChatComposer send and stop", () => {
  const base = {
    modelValue: "hi",
    provider: "cursor",
    model: null,
    providers: ["cursor"],
    models: {},
    providerLocked: false,
    blocked: null,
    attachments: [],
    attaching: false,
    references: [],
    autoApprove: false,
  };

  it("sends with UChatPromptSubmit while idle", async () => {
    const w = mount(ChatComposer, { props: { ...base, streaming: false } });
    const send = w.get("[data-testid='chat-send']");
    expect(send.attributes("aria-label")).toBe("Send");
    expect(w.find("[data-testid='chat-stop']").exists()).toBe(false);
    await send.trigger("click");
    expect(w.emitted("submit")).toEqual([["hi"]]);
  });

  it("queues and stops while a turn runs", async () => {
    const w = mount(ChatComposer, { props: { ...base, streaming: true } });
    expect(w.find("[data-testid='chat-send']").exists()).toBe(false);
    await w.get("[data-testid='chat-queue']").trigger("click");
    expect(w.emitted("submit")).toEqual([["hi"]]);
    const stop = w.get("[data-testid='chat-stop']");
    expect(stop.attributes("aria-label")).toBe("Stop");
    await stop.trigger("click");
    expect(w.emitted("stop")).toHaveLength(1);
  });
});
