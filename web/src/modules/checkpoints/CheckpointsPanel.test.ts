// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { defineComponent, h as el } from "vue";
import { TooltipProvider } from "reka-ui";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("@/lib/toast", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));
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
import UAvatar from "@nuxt/ui/components/Avatar.vue";

const DIFF = "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1,1 +1,1 @@\n-x\n+y\n";
let api: Record<string, ReturnType<typeof vi.fn>>;

const CHECKPOINTS = [
  { sha: "aaaaaaa111", summary: "checkpoint: initial", author: "Ada", ts: 100 },
  { sha: "bbbbbbb222", summary: "checkpoint: hero", author: "orso", ts: 200 },
];

async function mountPanel(
  start = "/p/p/history",
  log: { root: string; head: string | null; checkpoints?: unknown[] } = { root: ".", head: "bbbbbbb222" },
  extra: Record<string, unknown> = {},
) {
  api = {
    listCheckpoints: vi.fn(async () => ({ checkpoints: CHECKPOINTS, ...log })),
    checkpointDiff: vi.fn(async () => DIFF),
    restoreCheckpoint: vi.fn(async () => ({ restored: "aaaaaaa111" })),
    deployLink: vi.fn(async () => ({ url: null, state: "unknown" })),
    ...extra,
  };
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      {
        path: "/p/:id/history/:sha?",
        component: CheckpointsPanel,
        props: (r) => ({ projectId: r.params.id as string, sha: (r.params.sha as string) || undefined }),
      },
    ],
  });
  await router.push(start);
  // Tooltips need the provider `UApp` gives the real app.
  const Host = defineComponent({
    setup: () => () => el(TooltipProvider, null, { default: () => el(RouterView) }),
  });
  return mount(Host, { global: { plugins: [router] }, attachTo: document.body });
}

const byText = (text: string) =>
  [...document.body.querySelectorAll("button")].find((b) => b.textContent?.includes(text));

beforeEach(() => localStorage.clear());
afterEach(() => {
  document.body.innerHTML = "";
});

describe("CheckpointsPanel", () => {
  it("shows the timeline newest first", async () => {
    const w = await mountPanel();
    await flushPromises();
    expect(w.get("h1").text()).toBe("History");
    expect(w.find("[data-testid='history-scope']").exists()).toBe(false);
    const items = w.findAll("[data-testid='checkpoint-item']");
    expect(items.map((i) => i.text())).toEqual([
      expect.stringContaining("checkpoint: hero"),
      expect.stringContaining("checkpoint: initial"),
    ]);
    expect(w.findAll("[data-testid='checkpoint-author']").map((a) => a.text())).toEqual([
      "orso",
      "Ada",
    ]);
    w.unmount();
  });

  it("groups the timeline by day", async () => {
    const w = await mountPanel();
    await flushPromises();
    expect(w.findAll("[data-testid='checkpoint-day']")).toHaveLength(1);
    h.daemon.emit({
      type: "checkpoint.created", projectId: "p", sha: "ccccccc333", summary: "checkpoint: new",
      author: "Grace", ts: Math.floor(Date.now() / 1000),
    });
    await flushPromises();
    expect(w.findAll("[data-testid='checkpoint-day']").map((d) => d.text())).toEqual([
      "Today",
      expect.stringContaining("1970"),
    ]);
    w.unmount();
  });

  it("prepends checkpoint.created events", async () => {
    const w = await mountPanel();
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
    const w = await mountPanel();
    await flushPromises();
    api.listCheckpoints!.mockResolvedValueOnce({
      checkpoints: [{ sha: "ddddddd444", summary: "checkpoint: missed", author: "orso", ts: 300 }],
      root: ".",
      head: "ddddddd444",
    });
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
    const w = await mountPanel();
    await flushPromises();
    await w.findAll("[data-testid='checkpoint-item']")[1]!.trigger("click");
    await flushPromises();
    expect(api.checkpointDiff).toHaveBeenCalledWith("p", "aaaaaaa111");
    await vi.waitFor(() => expect(w.find("[data-testid='diff-stub']").exists()).toBe(true));
    expect(w.find("[data-testid='diff-stub']").text()).toBe("a.txt");
    expect(w.find("[data-testid='tree-stub']").text()).toBe("a.txt:modified");
    expect(w.vm.$route.path).toBe("/p/p/history/aaaaaaa111");
    w.unmount();
  });

  it("opens the checkpoint named in the URL and goes back to the timeline", async () => {
    const w = await mountPanel("/p/p/history/bbbbbbb222");
    await flushPromises();
    expect(api.checkpointDiff).toHaveBeenCalledWith("p", "bbbbbbb222");
    expect(w.find("[data-testid='timeline']").exists()).toBe(false);
    await w.find("[aria-label='Back to timeline']").trigger("click");
    await flushPromises();
    expect(w.vm.$route.path).toBe("/p/p/history");
    expect(w.find("[data-testid='timeline']").exists()).toBe(true);
    w.unmount();
  });

  it("restores only after confirming in the dialog", async () => {
    const w = await mountPanel();
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
    expect(w.vm.$route.path).toBe("/p/p/history");
    expect(w.find("[data-testid='timeline']").exists()).toBe(true);
    w.unmount();
  });

  it("disables restore for the current checkpoint", async () => {
    const w = await mountPanel();
    await flushPromises();
    await w.findAll("[data-testid='checkpoint-item']")[0]!.trigger("click");
    await flushPromises();
    expect(w.find("[data-testid='restore']").attributes("disabled")).toBeDefined();
    w.unmount();
  });

  it("searches by summary, author or sha", async () => {
    const w = await mountPanel();
    await flushPromises();
    const input = w.get("[data-testid='history-search']");
    await input.setValue("ada");
    expect(w.findAll("[data-testid='checkpoint-author']").map((a) => a.text())).toEqual(["Ada"]);
    await input.setValue("bbbbbbb");
    expect(w.findAll("[data-testid='checkpoint-author']").map((a) => a.text())).toEqual(["orso"]);
    await input.setValue("nothing like it");
    expect(w.find("[data-testid='history-no-match']").exists()).toBe(true);
    w.unmount();
  });

  it("filters by what went out once the daemon says, and shows authors' avatars", async () => {
    const w = await mountPanel("/p/p/history", {
      root: ".",
      head: "bbbbbbb222",
      checkpoints: [
        { ...CHECKPOINTS[0], published: true, avatarUrl: "https://avatars.githubusercontent.com/u/42?v=4" },
        { ...CHECKPOINTS[1], published: false },
      ],
    });
    await flushPromises();
    const authors = () => w.findAll("[data-testid='checkpoint-author']").map((a) => a.text());
    expect(authors()).toEqual(["orso", "Ada"]);
    // The picture loads lazily (no image in happy-dom); the avatar is given it, or initials.
    const avatars = w
      .findAllComponents(UAvatar)
      .filter((a) => a.props("alt") !== undefined);
    expect(avatars.map((a) => a.props("src"))).toEqual([undefined, "https://avatars.githubusercontent.com/u/42?v=4"]);
    expect(w.findAll("[data-testid^='history-filter-']").map((b) => b.text())).toEqual(["All", "In this change", "Live"]);
    await w.get("[data-testid='history-filter-published']").trigger("click");
    expect(authors()).toEqual(["Ada"]);
    await w.get("[data-testid='history-filter-unpublished']").trigger("click");
    expect(authors()).toEqual(["orso"]);
    w.unmount();
  });

  it("links published checkpoints to their deployed site, and shows a deploy still building", async () => {
    vi.useFakeTimers();
    const live = "c".repeat(40);
    const building = "d".repeat(40);
    const failed = "f".repeat(40);
    const draft = "e".repeat(40);
    let buildingDone = false;
    const deployLink = vi.fn(async (_p: string, sha: string) => {
      if (sha === live) return { url: "https://live.example.com", state: "live" };
      if (sha === building)
        return buildingDone ? { url: "https://new.example.com", state: "live" } : { url: null, state: "building" };
      if (sha === failed) return { url: null, state: "failed" };
      return { url: null, state: "unknown" };
    });
    const w = await mountPanel(
      "/p/p/history",
      {
        root: ".",
        head: draft,
        checkpoints: [
          { sha: live, summary: "Launch", author: "Ada", ts: 100, published: true },
          { sha: failed, summary: "Broken", author: "Ada", ts: 150, published: true },
          { sha: building, summary: "Just published", author: "Ada", ts: 200, published: true },
          { sha: draft, summary: "Draft", author: "Ada", ts: 300, published: false },
        ],
      },
      { deployLink },
    );
    await flushPromises();
    // Only published checkpoints are looked up.
    expect(deployLink.mock.calls.map((c) => c[1]).sort()).toEqual([building, failed, live].sort());
    const links = () => w.findAll("[data-testid='checkpoint-row-deploy']").map((l) => l.attributes("href"));
    expect(links()).toEqual(["https://live.example.com"]);
    expect(w.findAll("[data-testid='checkpoint-row-deploy-building']")).toHaveLength(1);
    expect(w.findAll("[data-testid='checkpoint-row-deploy-failed']")).toHaveLength(1);

    // Published before its site deployed: asked again until it is live.
    buildingDone = true;
    await vi.advanceTimersByTimeAsync(20_000);
    await flushPromises();
    expect(links()).toEqual(["https://new.example.com", "https://live.example.com"]);
    expect(w.find("[data-testid='checkpoint-row-deploy-building']").exists()).toBe(false);
    w.unmount();
    vi.useRealTimers();
  });

  it("offers no publish filter on main, where everything is live", async () => {
    const w = await mountPanel("/p/p/history", {
      root: ".",
      head: "bbbbbbb222",
      checkpoints: CHECKPOINTS.map((c) => ({ ...c, published: true })),
    });
    await flushPromises();
    expect(w.find("[data-testid='history-filter-published']").exists()).toBe(false);
    w.unmount();
  });

  it("offers no publish filter when the daemon doesn't say", async () => {
    const w = await mountPanel();
    await flushPromises();
    expect(w.find("[data-testid='history-filter-published']").exists()).toBe(false);
    w.unmount();
  });

  describe("with a site folder set", () => {
    const scoped = { root: "site", head: "fffffff999" };

    it("labels the timeline and marks no checkpoint current when HEAD is outside the site", async () => {
      const w = await mountPanel("/p/p/history", scoped);
      await flushPromises();
      expect(w.get("[data-testid='history-scope']").text()).toBe("Changes to site/");
      expect(w.findAll("[data-testid='checkpoint-item']")).toHaveLength(2);
      expect(w.text()).not.toContain("Current");
      await w.findAll("[data-testid='checkpoint-item']")[0]!.trigger("click");
      await flushPromises();
      expect(w.find("[data-testid='restore']").attributes("disabled")).toBeUndefined();
      await w.find("[data-testid='restore']").trigger("click");
      await flushPromises();
      expect(document.body.textContent).toContain("including files outside site/");
      w.unmount();
    });

    it("marks the newest checkpoint current when it is HEAD", async () => {
      const w = await mountPanel("/p/p/history", { root: "site", head: "bbbbbbb222" });
      await flushPromises();
      expect(w.findAll("[data-testid='checkpoint-item']")[0]!.element.parentElement!.textContent).toContain("Current");
      w.unmount();
    });

    it("reloads instead of prepending checkpoint.created events", async () => {
      const w = await mountPanel("/p/p/history", scoped);
      await flushPromises();
      h.daemon.emit({
        type: "checkpoint.created", projectId: "p", sha: "ccccccc333", summary: "checkpoint: outside",
        author: "Grace", ts: 400,
      });
      await flushPromises();
      expect(api.listCheckpoints).toHaveBeenCalledTimes(2);
      expect(w.text()).not.toContain("checkpoint: outside");
      w.unmount();
    });
  });
});
