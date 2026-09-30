// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("./highlight", async (orig) => ({
  ...(await orig<typeof import("./highlight")>()),
  highlightCode: async (code: string) => `<pre class="shiki"><code>${code}</code></pre>`,
}));

import FilesPanel from "./FilesPanel.vue";
import { useFilesStore } from "./store";

let api: Record<string, ReturnType<typeof vi.fn>>;

function mountPanel() {
  api = {
    tree: vi.fn(async (_p: string, path: string) =>
      path === ""
        ? {
            path: "",
            entries: [
              { name: "index.astro", path: "index.astro", kind: "file", size: 5 },
              { name: "src", path: "src", kind: "dir", size: null },
            ],
          }
        : {
            path,
            entries: [{ name: "hero.mdx", path: "src/hero.mdx", kind: "file", size: 9 }],
          },
    ),
    file: vi.fn(async (_p: string, path: string) =>
      path === "logo.png"
        ? { path, size: 3, content: null, binary: true }
        : { path, size: 9, content: "# Hello", binary: false },
    ),
  };
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  return mount(FilesPanel, { props: { projectId: "p" }, attachTo: document.body });
}

beforeEach(() => localStorage.clear());
afterEach(() => vi.useRealTimers());

describe("FilesPanel", () => {
  it("lists the root with directories first and expands folders lazily", async () => {
    const w = mountPanel();
    await flushPromises();
    const names = w.findAll("[data-testid^='tree-']").map((n) => n.text());
    expect(names).toEqual(["src", "index.astro"]);
    expect(api.tree).toHaveBeenCalledTimes(1);
    await w.find("[data-testid='tree-src']").trigger("click");
    await flushPromises();
    expect(api.tree).toHaveBeenLastCalledWith("p", "src");
    expect(w.find("[data-testid='tree-src/hero.mdx']").exists()).toBe(true);
    w.unmount();
  });

  it("shows file contents read-only", async () => {
    const w = mountPanel();
    await flushPromises();
    await w.find("[data-testid='tree-index.astro']").trigger("click");
    await flushPromises();
    expect(api.file).toHaveBeenCalledWith("p", "index.astro");
    expect(w.find("[data-testid='file-content']").text()).toContain("# Hello");
    expect(w.find("textarea").exists()).toBe(false);
    expect(w.find("[contenteditable]").exists()).toBe(false);
    expect(w.find("input").exists()).toBe(false);
    w.unmount();
  });

  it("explains binary files instead of rendering them", async () => {
    const w = mountPanel();
    await flushPromises();
    useFilesStore().dirs[""] = [{ name: "logo.png", path: "logo.png", kind: "file", size: 3 }];
    await flushPromises();
    await w.find("[data-testid='tree-logo.png']").trigger("click");
    await flushPromises();
    expect(w.find("[data-testid='file-unavailable']").text()).toContain("Binary file");
    w.unmount();
  });

  it("refreshes the tree and open file on fs.change (debounced)", async () => {
    const w = mountPanel();
    await flushPromises();
    await w.find("[data-testid='tree-index.astro']").trigger("click");
    await flushPromises();
    vi.useFakeTimers();
    h.daemon.emit({ type: "fs.change", projectId: "p", path: "index.astro", kind: "modify" });
    h.daemon.emit({ type: "fs.change", projectId: "p", path: "index.astro", kind: "modify" });
    h.daemon.emit({ type: "fs.change", projectId: "other", path: "x", kind: "modify" });
    await vi.advanceTimersByTimeAsync(300);
    expect(api.tree).toHaveBeenCalledTimes(2);
    expect(api.file).toHaveBeenCalledTimes(2);
    w.unmount();
  });
});
