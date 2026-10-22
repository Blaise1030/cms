// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { defineComponent, h as render } from "vue";
import { mount, flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import type { DevserverState } from "@/core/types";
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

it("refreshes only for content, assets, config, integration package and overflow paths", async () => {
  for (const path of ["src/content/pages/home.json", "src/content", "src/assets/blog/welcome/a.png", "astro.config.mjs", "src/content.config.ts", "packages/astro/src/index.js", ""]) expect(affectsContent(path), path).toBe(true);
  for (const path of ["src/content-schemas.mjs", "scripts/orso-content.mjs", "notes.txt", "src/pages/index.astro", "src/contentious.ts", "src/assets-old/x", "node_modules/x/index.js", "dist/index.html", ".git/HEAD"]) expect(affectsContent(path), path).toBe(false);
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

it("refreshes the catalog once per change in this project's dev server state", async () => {
  const status = (projectId: string, state: DevserverState) => h.daemon.emit({ type: "devserver.status", projectId, state });
  const store = useContentStore(); await store.open("p"); catalog.mockClear();
  status("other", "ready"); status("other", "crashed");
  vi.advanceTimersByTime(REFRESH_DELAY_MS); await flushPromises();
  expect(catalog).not.toHaveBeenCalled();
  status("p", "ready");
  vi.advanceTimersByTime(REFRESH_DELAY_MS); await flushPromises();
  expect(catalog).toHaveBeenCalledTimes(1);
  status("p", "crashed");
  vi.advanceTimersByTime(REFRESH_DELAY_MS); await flushPromises();
  expect(catalog).toHaveBeenCalledTimes(2);
  // A restart (or a stop then start) leaves ready through starting.
  status("p", "starting");
  vi.advanceTimersByTime(REFRESH_DELAY_MS); await flushPromises();
  expect(catalog).toHaveBeenCalledTimes(3);
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

it.each([
  ["field", { title: "Edited" }, "B", { fields: { title: "Edited" } }],
  ["body", { title: "W" }, "Edited body", { fields: {}, body: "Edited body" }],
])("keeps a dirty %s edit through refreshes while the preview is unavailable, then saves it", async (_edit, draftFields, draftBody, update) => {
  const fields = { type: "object", required: true, properties: { title: { type: "string", required: true } } };
  let editable = true;
  const listContentCollections = vi.fn(async () => [{ name: "blog", kind: "markdown", fields, editable, reason: editable ? null : "Start the preview to edit content" }]);
  const updateContentEntry = vi.fn(async () => ({ ...entry, fields: draftFields, body: draftBody, hash: "h2" }));
  h.daemon = createFakeDaemon({ listContentCollections, listContentEntries: async () => [entry], contentEntry: async () => entry, updateContentEntry });
  setActivePinia(createPinia());
  const store = useContentStore(); await store.open("p"); await store.selectCollection("blog"); await store.selectEntry("welcome");
  store.draftFields = { ...draftFields }; store.setBody(draftBody);
  expect(store.dirty).toBe(true);
  editable = false;
  change("src/content/blog/en/other.md"); vi.advanceTimersByTime(REFRESH_DELAY_MS); await flushPromises();
  expect(store.collections[0]?.editable).toBe(false);
  expect(store.draftFields).toEqual(draftFields); expect(store.draftBody).toBe(draftBody);
  expect(store.dirty).toBe(true); expect(store.changedOnDisk).toBe(false);
  expect(store.readOnlyReason).toBe("Start the preview to edit content");
  editable = true;
  h.daemon.emit({ type: "events.lagged" } as never); vi.advanceTimersByTime(REFRESH_DELAY_MS); await flushPromises();
  expect(store.draftFields).toEqual(draftFields); expect(store.draftBody).toBe(draftBody);
  expect(store.dirty).toBe(true); expect(store.readOnlyReason).toBeNull();
  await store.saveDraft();
  expect(updateContentEntry).toHaveBeenCalledWith(expect.objectContaining({ ...update, hash: "h1" }));
  expect(store.dirty).toBe(false);
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
