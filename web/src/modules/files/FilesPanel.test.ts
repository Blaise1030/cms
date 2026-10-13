// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import { defineComponent, h } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";

const hDaemon = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => hDaemon.daemon }));
vi.mock("./FileCodeMirror.vue", () => ({
  // FileViewer loads this via defineAsyncComponent, which unwraps `default` only for ES modules.
  __esModule: true,
  default: defineComponent({
    name: "FileCodeMirror",
    props: { code: { type: String, required: true }, path: { type: String, required: true } },
    setup: (props) => () => h("pre", { "data-testid": "file-content" }, props.code),
  }),
}));
vi.mock("./ProjectFilesTree.vue", () => ({
  default: defineComponent({
    name: "ProjectFilesTree",
    props: { paths: { type: Array, required: true }, selectedPath: { type: String, default: null } },
    emits: ["select"],
    setup(props, { emit }) {
      return () =>
        h(
          "div",
          { "data-testid": "project-files-tree", role: "tree", "aria-label": "Project files" },
          (props.paths as string[]).map((path) =>
            h(
              "button",
              {
                type: "button",
                "data-testid": `tree-${path}`,
                "data-item-path": path,
                onClick: () => emit("select", path),
              },
              path.split("/").at(-1),
            ),
          ),
        );
    },
  }),
}));

import FilesPanel from "./FilesPanel.vue";
import { useFilesStore } from "./store";

let api: Record<string, ReturnType<typeof vi.fn>>;

async function mountPanel(start = "/p/p/code") {
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
  hDaemon.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      {
        path: "/p/:id/code/:path(.*)?",
        component: FilesPanel,
        props: (r) => ({ projectId: r.params.id as string, path: (r.params.path as string) || undefined }),
      },
      { path: "/other", component: { template: "<p>Other</p>" } },
    ],
  });
  await router.push(start);
  await router.isReady();
  return mount(RouterView, { global: { plugins: [router] }, attachTo: document.body });
}

beforeEach(() => localStorage.clear());
afterEach(() => {
  document.body.innerHTML = "";
  vi.useRealTimers();
});

describe("FilesPanel", () => {
  it("walks the tree into flat paths for the Pierre file tree", async () => {
    const w = await mountPanel();
    await flushPromises();
    expect(api.tree).toHaveBeenCalledWith("p", "");
    expect(api.tree).toHaveBeenCalledWith("p", "src");
    expect(useFilesStore().paths).toEqual(["index.astro", "src/hero.mdx"]);
    expect(w.find("[data-testid='tree-index.astro']").text()).toBe("index.astro");
    expect(w.find("[data-testid='tree-src/hero.mdx']").text()).toBe("hero.mdx");
    w.unmount();
  });

  it("shows file contents read-only", async () => {
    const w = await mountPanel();
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

  it("puts the selected file in the URL and opens the file named there", async () => {
    const w = await mountPanel();
    await flushPromises();
    await w.find("[data-testid='tree-src/hero.mdx']").trigger("click");
    await flushPromises();
    expect(w.vm.$route.path).toBe("/p/p/code/src/hero.mdx");
    w.unmount();

    const reloaded = await mountPanel("/p/p/code/src/hero.mdx");
    await flushPromises();
    expect(api.file).toHaveBeenCalledWith("p", "src/hero.mdx");
    expect(reloaded.find("[data-testid='file-content']").text()).toContain("# Hello");
    reloaded.unmount();
  });

  it("explains binary files instead of rendering them", async () => {
    const w = await mountPanel();
    await flushPromises();
    useFilesStore().paths = ["logo.png"];
    await flushPromises();
    await w.find("[data-testid='tree-logo.png']").trigger("click");
    await flushPromises();
    expect(w.find("[data-testid='file-unavailable']").text()).toContain("Binary file");
    w.unmount();
  });

  it("refreshes the tree and open file on events.lagged", async () => {
    const w = await mountPanel();
    await flushPromises();
    await w.find("[data-testid='tree-index.astro']").trigger("click");
    await flushPromises();
    const treesBefore = api.tree.mock.calls.length;
    hDaemon.daemon.emit({ type: "events.lagged", missed: 9 });
    await flushPromises();
    expect(api.tree.mock.calls.length).toBeGreaterThan(treesBefore);
    expect(api.file).toHaveBeenCalledTimes(2);
    w.unmount();
  });

  it("refreshes the tree and open file on fs.change (debounced)", async () => {
    const w = await mountPanel();
    await flushPromises();
    await w.find("[data-testid='tree-index.astro']").trigger("click");
    await flushPromises();
    const treesBefore = api.tree.mock.calls.length;
    vi.useFakeTimers();
    hDaemon.daemon.emit({ type: "fs.change", projectId: "p", path: "index.astro", kind: "modify" });
    hDaemon.daemon.emit({ type: "fs.change", projectId: "p", path: "index.astro", kind: "modify" });
    hDaemon.daemon.emit({ type: "fs.change", projectId: "other", path: "x", kind: "modify" });
    await vi.advanceTimersByTimeAsync(300);
    expect(api.tree.mock.calls.length).toBeGreaterThan(treesBefore);
    expect(api.file).toHaveBeenCalledTimes(2);
    w.unmount();
  });
});
