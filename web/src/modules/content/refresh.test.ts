// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { defineComponent, h as render } from "vue";
import { mount, flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
import { REFRESH_DELAY_MS, affectsContent, useContentStore } from "./store";

const entry = { collection: "blog", id: "welcome", path: "src/content/blog/en/welcome.md", kind: "markdown", source: "S", fields: { title: "W" }, body: "B", branch: "content/a", hash: "h1" };
let catalog: ReturnType<typeof vi.fn>;
beforeEach(() => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  catalog = vi.fn(async () => [{ name: "blog", kind: "markdown", editable: true, reason: null }]);
  h.daemon = createFakeDaemon({ listContentCollections: catalog, listContentEntries: async () => [entry], contentEntry: async () => entry });
  setActivePinia(createPinia());
});
afterEach(() => { vi.useRealTimers(); });
const change = (path: string) => h.daemon.emit({ type: "fs.change", projectId: "p", path, kind: "modify" });

it("coalesces a burst of content file events into one trailing refresh", async () => {
  const store = useContentStore(); await store.open("p"); catalog.mockClear();
  for (let i = 0; i < 200; i++) change(`src/content/blog/en/post-${i}.md`);
  await flushPromises(); expect(catalog).not.toHaveBeenCalled();
  vi.advanceTimersByTime(REFRESH_DELAY_MS - 1); await flushPromises(); expect(catalog).not.toHaveBeenCalled();
  change("src/content/site.json");
  vi.advanceTimersByTime(REFRESH_DELAY_MS); await flushPromises();
  expect(catalog).toHaveBeenCalledTimes(1);
});

it("refreshes only for content, assets, schema, helper and overflow paths", async () => {
  for (const path of ["src/content/pages/home.json", "src/content", "src/assets/blog/welcome/a.png", "src/content-schemas.mjs", "scripts/orso-content.mjs", ""]) expect(affectsContent(path), path).toBe(true);
  for (const path of ["notes.txt", "src/pages/index.astro", "src/contentious.ts", "src/assets-old/x", "node_modules/x/index.js", "dist/index.html", ".git/HEAD"]) expect(affectsContent(path), path).toBe(false);
  const store = useContentStore(); await store.open("p"); catalog.mockClear();
  change("src/pages/index.astro"); change("dist/index.html");
  vi.advanceTimersByTime(REFRESH_DELAY_MS); await flushPromises();
  expect(catalog).not.toHaveBeenCalled();
  change("");
  vi.advanceTimersByTime(REFRESH_DELAY_MS); await flushPromises();
  expect(catalog).toHaveBeenCalledTimes(1);
});

it("stops refreshing while the Content panel is unmounted and refreshes once on remount", async () => {
  const Panel = defineComponent({ setup() { const store = useContentStore(); void store.open("p"); return () => render("div"); }, unmounted() { useContentStore().suspend(); } });
  const first = mount(Panel); await flushPromises(); catalog.mockClear();
  first.unmount();
  change("src/content/site.json"); h.daemon.emit({ type: "events.lagged" } as never); h.daemon.simulateReconnect();
  vi.advanceTimersByTime(REFRESH_DELAY_MS * 4); await flushPromises();
  expect(catalog).not.toHaveBeenCalled();
  const second = mount(Panel); await flushPromises();
  expect(catalog).toHaveBeenCalledTimes(1);
  vi.advanceTimersByTime(REFRESH_DELAY_MS * 4); await flushPromises();
  expect(catalog).toHaveBeenCalledTimes(1);
  second.unmount();
});

it("still preserves a dirty draft and flags the on-disk change after a debounced refresh", async () => {
  let onDisk = entry;
  h.daemon = createFakeDaemon({ listContentCollections: catalog, listContentEntries: async () => [entry], contentEntry: async () => onDisk });
  setActivePinia(createPinia());
  const store = useContentStore(); await store.open("p"); await store.selectCollection("blog"); await store.selectEntry("welcome");
  store.draftSource = "Draft";
  onDisk = { ...entry, source: "Changed", hash: "h2" };
  change(entry.path); vi.advanceTimersByTime(REFRESH_DELAY_MS); await flushPromises();
  expect(store.draftSource).toBe("Draft"); expect(store.changedOnDisk).toBe(true);
});

it("loads reference picker entries once per collection and reloads them in place on refresh", async () => {
  const listEntries = vi.fn(async (_p: string, collection: string) => collection === "sections" ? [{ ...entry, collection, id: "hero" }, { ...entry, collection, id: "faq" }] : [entry]);
  h.daemon = createFakeDaemon({ listContentCollections: catalog, listContentEntries: listEntries, contentEntry: async () => entry });
  setActivePinia(createPinia());
  const store = useContentStore(); await store.open("p");
  await Promise.all([store.loadReferenceEntries("sections"), store.loadReferenceEntries("sections")]);
  expect(store.referenceEntries.sections).toEqual(["hero", "faq"]);
  expect(listEntries.mock.calls.filter(([, c]) => c === "sections")).toHaveLength(1);
  listEntries.mockImplementation(async (_p: string, collection: string) => collection === "sections" ? [{ ...entry, collection, id: "hero" }, { ...entry, collection, id: "new" }] : [entry]);
  change("src/content/sections/new.mdx"); vi.advanceTimersByTime(REFRESH_DELAY_MS);
  // The picker keeps its list while the refresh reloads it.
  expect(store.referenceEntries.sections).toEqual(["hero", "faq"]);
  await flushPromises();
  expect(store.referenceEntries.sections).toEqual(["hero", "new"]);
});
