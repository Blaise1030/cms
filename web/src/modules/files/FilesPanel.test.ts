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

/** A `/api/tree` stub over a flat list of repo-relative files. */
function treeOf(files: string[]) {
  return vi.fn(async (_p: string, dir: string) => {
    const prefix = dir ? `${dir}/` : "";
    const entries = new Map<string, { name: string; path: string; kind: "file" | "dir"; size: number | null }>();
    for (const file of files) {
      if (!file.startsWith(prefix)) continue;
      const [name, ...rest] = file.slice(prefix.length).split("/");
      const path = `${prefix}${name}`;
      entries.set(path, { name: name!, path, kind: rest.length ? "dir" : "file", size: rest.length ? null : 1 });
    }
    return { path: dir, entries: [...entries.values()] };
  });
}

const SCOPED_FILES = ["README.md", "crates/cli/main.rs", "site/index.astro", "site/pages/a.md"];

async function mountScoped(siteRoot: string, start = "/p/p/code") {
  const w = await mountPanel(start, {
    tree: treeOf(SCOPED_FILES),
    settings: vi.fn(async () => ({ siteRoot })),
  });
  await flushPromises();
  return w;
}

async function mountPanel(start = "/p/p/code", overrides: Record<string, ReturnType<typeof vi.fn>> = {}) {
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
    ...overrides,
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

  describe("site folder from orso.json", () => {
    const treeProps = (w: Awaited<ReturnType<typeof mountPanel>>) =>
      w.findComponent({ name: "ProjectFilesTree" }).props() as { paths: string[]; selectedPath: string | null };

    it("walks the whole repo when the root is '.'", async () => {
      const w = await mountScoped(".");
      expect(api.tree).toHaveBeenCalledWith("p", "");
      expect(useFilesStore().root).toBe("");
      expect(useFilesStore().paths).toEqual(SCOPED_FILES);
      expect(treeProps(w).paths).toEqual(SCOPED_FILES);
      w.unmount();
    });

    it("walks only the site folder and shows the tree rooted there", async () => {
      const w = await mountScoped("site");
      expect(api.tree.mock.calls.map((c) => c[1])).toEqual(["site", "site/pages"]);
      expect(useFilesStore().paths).toEqual(["site/index.astro", "site/pages/a.md"]);
      expect(treeProps(w).paths).toEqual(["index.astro", "pages/a.md"]);

      await w.find("[data-testid='tree-pages/a.md']").trigger("click");
      await flushPromises();
      expect(w.vm.$route.path).toBe("/p/p/code/site/pages/a.md");
      expect(api.file).toHaveBeenCalledWith("p", "site/pages/a.md");
      expect(treeProps(w).selectedPath).toBe("pages/a.md");
      w.unmount();
    });

    it("opens a site file named in the URL by its repo path", async () => {
      const w = await mountScoped("site", "/p/p/code/site/index.astro");
      expect(api.file).toHaveBeenCalledWith("p", "site/index.astro");
      expect(treeProps(w).selectedPath).toBe("index.astro");
      w.unmount();
    });

    it("falls back to the repo root when settings cannot be read", async () => {
      const w = await mountPanel("/p/p/code", {
        tree: treeOf(SCOPED_FILES),
        settings: vi.fn(async () => { throw new Error("boom"); }),
      });
      await flushPromises();
      expect(useFilesStore().paths).toEqual(SCOPED_FILES);
      w.unmount();
    });

    it("re-reads the site folder when files change", async () => {
      const w = await mountScoped("site");
      api.settings!.mockResolvedValue({ siteRoot: "." });
      hDaemon.daemon.emit({ type: "events.lagged", missed: 1 });
      await flushPromises();
      expect(useFilesStore().paths).toEqual(SCOPED_FILES);
      expect(treeProps(w).paths).toEqual(SCOPED_FILES);
      w.unmount();
    });
  });

  it("lists sibling folders in parallel, at most 8 requests at once", async () => {
    const files = Array.from({ length: 20 }, (_, i) => `d${i}/f.txt`);
    const list = treeOf(files);
    let active = 0;
    let peak = 0;
    const w = await mountPanel("/p/p/code", {
      tree: vi.fn(async (p: string, dir: string) => {
        active++;
        peak = Math.max(peak, active);
        await new Promise((r) => setTimeout(r, 1));
        active--;
        return list(p, dir);
      }),
    });
    await vi.waitFor(() => expect(useFilesStore().paths).toHaveLength(20));
    expect(useFilesStore().paths).toEqual(files);
    expect(peak).toBeGreaterThan(1);
    expect(peak).toBeLessThanOrEqual(8);
    w.unmount();
  });
});
