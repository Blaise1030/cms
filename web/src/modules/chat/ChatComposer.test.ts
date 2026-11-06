// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import ChatComposer from "./ChatComposer.vue";

enableAutoUnmount(afterEach);

/** Render menus inline; items emit `select` on click, as the real ones do. */
const stubs = {
  DropdownMenu: { template: "<div><slot /></div>" },
  DropdownMenuTrigger: { template: "<div><slot /></div>" },
  DropdownMenuContent: { template: "<div><slot /></div>" },
  DropdownMenuLabel: { template: "<div><slot /></div>" },
  DropdownMenuSeparator: { template: "<hr />" },
  DropdownMenuSub: { template: "<div><slot /></div>" },
  DropdownMenuSubTrigger: { template: "<div><slot /></div>" },
  DropdownMenuSubContent: { template: "<div><slot /></div>" },
  DropdownMenuItem: {
    emits: ["select"],
    template: `<div @click="$emit('select')"><slot /></div>`,
  },
  Command: { template: "<div><slot /></div>" },
  CommandInput: { template: "<input />" },
  CommandList: { template: "<div><slot /></div>" },
  CommandEmpty: { template: "<div><slot /></div>" },
  CommandGroup: {
    props: ["heading"],
    template: `<div><div v-if="heading" data-slot="command-group-heading">{{ heading }}</div><slot /></div>`,
  },
  CommandItem: {
    emits: ["select"],
    template: `<div @click="$emit('select')"><slot /></div>`,
  },
};

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

  it("offers both modes and emits the picked one", async () => {
    const w = mountComposer(false);
    expect(w.find("[data-testid='chat-approval-ask']").text()).toContain("Ask each time");
    expect(w.find("[data-testid='chat-approval-auto']").text()).toContain("Auto-approve");
    await w.find("[data-testid='chat-approval-auto']").trigger("click");
    await w.find("[data-testid='chat-approval-ask']").trigger("click");
    expect(w.emitted("setAutoApprove")).toEqual([[true], [false]]);
  });

  it("marks the mode in force", () => {
    expect(mountComposer(true).find("[data-testid='chat-approval-auto'] svg").exists()).toBe(true);
    expect(mountComposer(true).find("[data-testid='chat-approval-ask'] svg").exists()).toBe(false);
  });
});

describe("ChatComposer assistant setup", () => {
  it("ends the assistant list with a setup action and emits it", async () => {
    const w = mountComposer(false);
    await flushPromises();
    const item = w.find("[data-testid='chat-agent-setup']");
    expect(item.text()).toContain("Set up another assistant…");
    await item.trigger("click");
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
    });
    await flushPromises();

    expect(w.find("[data-testid='chat-agent-sub'][data-provider='opencode']").exists()).toBe(true);
    expect(w.find("[data-testid='chat-model-search']").exists()).toBe(true);

    const subs = [...w.element.querySelectorAll("[data-testid='chat-model-sub']")].map((el) =>
      el.getAttribute("data-model-provider"),
    );
    expect(subs).toEqual(["deepseek", "opencode-go"]);

    const topLabels = [...w.element.querySelectorAll("[data-testid='chat-model-item']")].map((el) =>
      el.querySelector("[data-testid='chat-model-label']")?.textContent?.trim(),
    );
    expect(topLabels).toContain("Default");
    expect(topLabels).not.toContain("deepseek-flash");

    const nested = [...w.element.querySelectorAll("[data-testid='chat-model-sub-item']")].map((el) =>
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
    });
    await flushPromises();

    const agents = [...w.element.querySelectorAll("[data-testid='chat-agent-sub']")].map((el) =>
      el.getAttribute("data-provider"),
    );
    expect(agents).toEqual(["cursor", "claude", "opencode"]);
    expect(w.find("[data-testid='chat-model-item'][data-value='cursor:default']").exists()).toBe(true);
    expect(w.find("[data-testid='chat-model-sub'][data-model-provider='deepseek']").exists()).toBe(true);
  });
});
