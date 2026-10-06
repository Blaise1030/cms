// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";

const h = vi.hoisted(() => ({ render: vi.fn(), cleanUp: vi.fn() }));
vi.mock("@pierre/diffs", () => ({
  FileDiff: class {
    render = h.render;
    cleanUp = h.cleanUp;
  },
}));

import DiffView from "./DiffView.vue";

describe("DiffView", () => {
  it("renders the file diff into its host and cleans up on unmount", async () => {
    const diff = { name: "a.txt" } as never;
    const w = mount(DiffView, { props: { diff }, attachTo: document.body });
    expect(h.render).toHaveBeenCalledWith({
      fileDiff: diff,
      containerWrapper: w.find("[data-testid='diff-view']").element,
    });
    const next = { name: "b.txt" } as never;
    await w.setProps({ diff: next });
    expect(h.render).toHaveBeenLastCalledWith(expect.objectContaining({ fileDiff: next }));
    w.unmount();
    expect(h.cleanUp).toHaveBeenCalled();
  });
});
