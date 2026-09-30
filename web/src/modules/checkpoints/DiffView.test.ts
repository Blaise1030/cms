// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import DiffView from "./DiffView.vue";

describe("DiffView", () => {
  it("renders a read-only merge view highlighting the changed lines", () => {
    const w = mount(DiffView, {
      props: { oldText: "a\nold line\nc", newText: "a\nnew line\nc" },
      attachTo: document.body,
    });
    const content = w.find(".cm-content");
    expect(content.exists()).toBe(true);
    expect(content.text()).toContain("new line");
    expect(content.attributes("contenteditable")).toBe("false");
    // Removed text is shown in a deleted-chunk widget, inserted text is marked as changed.
    expect(w.find(".cm-deletedChunk").text()).toContain("old line");
    expect(w.find(".cm-changedLine").exists()).toBe(true);
    w.unmount();
  });
});
