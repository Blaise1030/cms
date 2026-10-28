// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import ChatComposer from "./ChatComposer.vue";

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

function mountComposer(autoApprove: boolean) {
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
      attachments: [],
      attaching: false,
      references: [],
      autoApprove,
    },
    global: { stubs },
  });
}

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
