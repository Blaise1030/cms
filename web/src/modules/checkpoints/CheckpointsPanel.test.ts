// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("vue-sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));
// The panel loads these lazily; stand-ins keep Pierre's renderers out of happy-dom.
vi.mock("./DiffView.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    __esModule: true,
    default: defineComponent({
      props: { diff: { type: Object, required: true } },
      setup: (p) => () => h("pre", { "data-testid": "diff-stub" }, p.diff.name),
    }),
  };
});
vi.mock("./ChangedFilesTree.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    __esModule: true,
    default: defineComponent({
      props: { files: { type: Array, required: true } },
      setup: (p) => () =>
        h(
          "ul",
          { "data-testid": "tree-stub" },
          (p.files as { path: string; status: string }[]).map((f) => h("li", `${f.path}:${f.status}`)),
        ),
    }),
  };
});

import CheckpointsPanel from "./CheckpointsPanel.vue";
import { useCheckpointsStore } from "./store";

const DIFF = "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1,1 +1,1 @@\n-x\n+y\n";
let api: Record<string, ReturnType<typeof vi.fn>>;

function mountPanel() {
  api = {
    listCheckpoints: vi.fn(async () => [
      { sha: "aaaaaaa111", summary: "checkpoint: initial", author: "Ada", ts: 100 },
      { sha: "bbbbbbb222", summary: "checkpoint: hero", author: "Workbench", ts: 200 },
    ]),
    checkpointDiff: vi.fn(async () => DIFF),
    restoreCheckpoint: vi.fn(async () => ({ restored: "aaaaaaa111" })),
  };
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  return mount(CheckpointsPanel, {
    props: { projectId: "p" },
    attachTo: document.body,
  });
}

const byText = (text: string) =>
  [...document.body.querySelectorAll("button")].find((b) => b.textContent?.includes(text));

beforeEach(() => localStorage.clear());
afterEach(() => {
  document.body.innerHTML = "";
});

describe("CheckpointsPanel", () => {
  it("shows the timeline newest first", async () => {
    const w = mountPanel();
    await flushPromises();
    const items = w.findAll("[data-testid='checkpoint-item']");
    expect(items.map((i) => i.text())).toEqual([
      expect.stringContaining("checkpoint: hero"),
      expect.stringContaining("checkpoint: initial"),
    ]);
    expect(w.findAll("[data-testid='checkpoint-author']").map((a) => a.text())).toEqual([
      "Workbench",
      "Ada",
    ]);
    w.unmount();
  });

  it("prepends checkpoint.created events", async () => {
    const w = mountPanel();
    await flushPromises();
    h.daemon.emit({
      type: "checkpoint.created", projectId: "p", sha: "ccccccc333", summary: "checkpoint: new",
      author: "Grace", ts: 400,
    });
    await flushPromises();
    expect(w.findAll("[data-testid='checkpoint-item']")[0]!.text()).toContain("checkpoint: new");
    expect(w.findAll("[data-testid='checkpoint-author']")[0]!.text()).toBe("Grace");
    w.unmount();
  });

  it("reloads the timeline on events.lagged and reconnect", async () => {
    const w = mountPanel();
    await flushPromises();
    api.listCheckpoints!.mockResolvedValueOnce([
      { sha: "ddddddd444", summary: "checkpoint: missed", author: "Workbench", ts: 300 },
    ]);
    h.daemon.emit({ type: "events.lagged", missed: 2 });
    await flushPromises();
    expect(api.listCheckpoints).toHaveBeenCalledTimes(2);
    expect(w.findAll("[data-testid='checkpoint-item']")).toHaveLength(1);
    h.daemon.simulateReconnect();
    await flushPromises();
    expect(api.listCheckpoints).toHaveBeenCalledTimes(3);
    w.unmount();
  });

  it("opens a diff for the clicked checkpoint", async () => {
    const w = mountPanel();
    await flushPromises();
    await w.findAll("[data-testid='checkpoint-item']")[1]!.trigger("click");
    await flushPromises();
    expect(api.checkpointDiff).toHaveBeenCalledWith("p", "aaaaaaa111");
    await vi.waitFor(() => expect(w.find("[data-testid='diff-stub']").exists()).toBe(true));
    expect(w.find("[data-testid='diff-stub']").text()).toBe("a.txt");
    expect(w.find("[data-testid='tree-stub']").text()).toBe("a.txt:modified");
    w.unmount();
  });

  it("restores only after confirming in the dialog", async () => {
    const w = mountPanel();
    await flushPromises();
    await w.findAll("[data-testid='checkpoint-item']")[1]!.trigger("click");
    await flushPromises();
    await w.find("[data-testid='restore']").trigger("click");
    await flushPromises();
    expect(api.restoreCheckpoint).not.toHaveBeenCalled();
    expect(document.body.textContent).toContain("Restore this checkpoint?");
    byText("Cancel")!.click();
    await flushPromises();
    expect(api.restoreCheckpoint).not.toHaveBeenCalled();

    await w.find("[data-testid='restore']").trigger("click");
    await flushPromises();
    (document.body.querySelector("[data-testid='confirm-restore']") as HTMLElement).click();
    await flushPromises();
    expect(api.restoreCheckpoint).toHaveBeenCalledWith("p", "aaaaaaa111");
    // Timeline is reloaded and the detail view closes.
    expect(api.listCheckpoints).toHaveBeenCalledTimes(2);
    expect(useCheckpointsStore().selectedSha).toBeNull();
    expect(w.find("[data-testid='timeline']").exists()).toBe(true);
    w.unmount();
  });

  it("disables restore for the current checkpoint", async () => {
    const w = mountPanel();
    await flushPromises();
    await w.findAll("[data-testid='checkpoint-item']")[0]!.trigger("click");
    await flushPromises();
    expect(w.find("[data-testid='restore']").attributes("disabled")).toBeDefined();
    w.unmount();
  });
});
