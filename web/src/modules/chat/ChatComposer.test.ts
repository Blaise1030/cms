// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import ChatComposer from "./ChatComposer.vue";

enableAutoUnmount(afterEach);

/** Render the menu inline; items emit `select` on click, as the real one does. */
const stubs = {
  DropdownMenu: { template: "<div><slot /></div>" },
  DropdownMenuTrigger: { template: "<div><slot /></div>" },
  DropdownMenuContent: { template: "<div><slot /></div>" },
  DropdownMenuLabel: { template: "<div><slot /></div>" },
  DropdownMenuItem: {
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

const setupItem = () => document.body.querySelector<HTMLElement>("[data-testid='chat-agent-setup']");

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
    await w.find("[data-testid='chat-agent']").trigger("click");
    await flushPromises();
    const item = setupItem();
    expect(item?.textContent).toContain("Set up another assistant…");
    item!.click();
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
