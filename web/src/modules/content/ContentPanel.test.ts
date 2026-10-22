// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { RouterView } from "vue-router";
import { contentRouter } from "@/test/contentRouter";
import { mount, flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("./ContentTree.vue", () => import("@/test/contentTreeStub"));
import { REFRESH_DELAY_MS, useContentStore } from "./store";
beforeEach(() => { vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] }); });
afterEach(() => { vi.useRealTimers(); });
/** Emit a file event and let its debounced refresh start. */
function fsChange(path: string) { h.daemon.emit({type:"fs.change",projectId:"p",path,kind:"modify"}); vi.advanceTimersByTime(REFRESH_DELAY_MS); }
const entry = { collection: "blog", id: "welcome", path: "src/content/blog/en/welcome.md", kind: "markdown", source: "Original", fields: {title: "Welcome"}, body: "Original", branch: "main", hash: "abc" };
async function setup(api: Record<string, unknown> = {}, start = "/p/p/content") {
 h.daemon = createFakeDaemon({ listContentCollections: async () => [{name:"blog",kind:"markdown",editable:true,reason:null}], listContentEntries: async () => [entry], contentEntry: async () => entry, ...api });
 setActivePinia(createPinia());
 return mount(RouterView, {global:{plugins:[await contentRouter(start)]}});
}
it("navigates collections and entries and reads content on main", async () => {
 const w = await setup(); await flushPromises();
 await w.get('[data-testid="content-collection"]').trigger("click"); await flushPromises();
 await w.get('[data-testid="content-entry"]').trigger("click"); await flushPromises();
 expect(w.vm.$route.path).toBe("/p/p/content/blog/welcome");
 expect(w.text()).toContain("main"); expect(w.get('[data-testid="content-source"]').text()).toBe("Original");
 w.unmount();
});
it("opens the collection and entry named in the URL", async () => {
 const contentEntry = vi.fn(async () => entry);
 const w = await setup({ contentEntry }, "/p/p/content/blog/welcome"); await flushPromises();
 expect(contentEntry).toHaveBeenCalledWith("p", "blog", "welcome");
 expect(w.get('[data-testid="content-source"]').text()).toBe("Original");
 expect(w.get('[data-testid="content-entry"]').text()).toBe("welcome");
 w.unmount();
});
it("puts the entry left open back in the URL when the tab reopens", async () => {
 const w = await setup({}, "/p/p/content/blog/welcome"); await flushPromises();
 await w.vm.$router.push("/other"); await w.vm.$router.push("/p/p/content"); await flushPromises();
 expect(w.vm.$route.path).toBe("/p/p/content/blog/welcome");
 w.unmount();
});
it("preserves a dirty draft on filesystem refresh and cancelled navigation", async () => {
 let onDisk = entry;
 const w = await setup({ contentEntry: async () => onDisk }); await flushPromises(); const store = useContentStore();
 await store.selectCollection("blog"); await store.selectEntry("welcome"); store.draftSource = "Draft";
 // An unrelated file event does not flag the draft; a change to its file does.
 fsChange("notes.txt"); await flushPromises();
 expect(store.changedOnDisk).toBe(false);
 onDisk = {...entry, source: "Changed", hash: "def"};
 fsChange(entry.path); await flushPromises();
 expect(store.draftSource).toBe("Draft"); expect(store.dirty).toBe(true); expect(store.changedOnDisk).toBe(true);
 vi.stubGlobal("confirm", vi.fn(() => false)); await store.selectCollection("blog"); expect(store.entry?.id).toBe("welcome");
 vi.unstubAllGlobals(); w.unmount();
});
it("blocks leaving the Content tab until a dirty draft is discarded", async () => {
 const w = await setup(); await flushPromises(); const store = useContentStore();
 await store.selectCollection("blog"); await store.selectEntry("welcome"); store.draftSource = "Draft";
 vi.stubGlobal("confirm", vi.fn(() => false));
 await w.vm.$router.push("/other");
 expect(w.vm.$route.path).toBe("/p/p/content"); expect(store.draftSource).toBe("Draft");
 vi.stubGlobal("confirm", vi.fn(() => true));
 await w.vm.$router.push("/other"); expect(w.vm.$route.path).toBe("/other");
 expect(store.dirty).toBe(false); expect(store.draftSource).toBe("Original");
 vi.unstubAllGlobals(); w.unmount();
});

function deferred<T>() {
 let resolve!: (value: T) => void;
 const promise = new Promise<T>(done => { resolve = done; });
 return {promise, resolve};
}
it.each(["selection-first", "refresh-first"])("keeps the user's entry selection and clears loading during %s overlap", async (ordering) => {
 const second = {...entry, id:"second", source:"Second source", hash:"second"};
 const selection = deferred<typeof entry>();
 const catalog = deferred<{name:string;kind:string;editable:boolean;reason:null}[]>();
 let holdCatalog = false;
 const w = await setup({
  listContentCollections: async () => holdCatalog ? catalog.promise : [{name:"blog",kind:"markdown",editable:true,reason:null}],
  contentEntry: async (_project: string, _collection: string, id: string) => id === "second" ? selection.promise : entry,
 });
 await flushPromises(); const store = useContentStore();
 await store.selectCollection("blog"); await store.selectEntry("welcome");
 holdCatalog = true;
 let chosen: Promise<void>;
 if (ordering === "selection-first") {
  chosen = store.selectEntry("second");
  fsChange(entry.path);
 } else {
  fsChange(entry.path);
  chosen = store.selectEntry("second");
 }
 await flushPromises(); expect(store.loading).toBe(true);
 selection.resolve(second); await chosen;
 catalog.resolve([{name:"blog",kind:"markdown",editable:true,reason:null}]); await flushPromises();
 expect(store.entry?.id).toBe("second");
 expect(store.draftSource).toBe("Second source");
 expect(store.loading).toBe(false);
 expect(w.text()).not.toContain("Loading content…");
 w.unmount();
});
it("keeps showing a draft's form with Save disabled while the preview is unavailable", async () => {
 const site = { collection: "site", id: "site", path: "src/content/site.json", kind: "json", source: "{}", fields: { title: "Site" }, branch: "content/a", hash: "h1" };
 const fields = { type: "object", required: true, properties: { title: { type: "string", required: true } } };
 let editable = true;
 const w = await setup({
  listContentCollections: async () => [{ name: "site", kind: "json", fields, editable, reason: editable ? null : "Start the preview to edit content" }],
  listContentEntries: async () => [site], contentEntry: async () => site,
 }, "/p/p/content/site/site"); await flushPromises();
 await w.get('[data-testid="field-title"]').setValue("Edited"); await flushPromises();
 expect(w.get('[data-testid="content-save"]').attributes("disabled")).toBeUndefined();
 editable = false;
 fsChange("src/content/blog/en/other.md"); await flushPromises();
 expect(w.find('[data-testid="content-form"]').exists()).toBe(true);
 expect((w.get('[data-testid="field-title"]').element as HTMLInputElement).value).toBe("Edited");
 expect(w.get('[data-testid="content-read-only"]').text()).toBe("Start the preview to edit content");
 expect(w.get('[data-testid="content-save"]').attributes("disabled")).toBeDefined();
 editable = true;
 fsChange("src/content/blog/en/other.md"); await flushPromises();
 expect(w.find('[data-testid="content-read-only"]').exists()).toBe(false);
 expect(w.get('[data-testid="content-save"]').attributes("disabled")).toBeUndefined();
 w.unmount();
});
it("keeps supported collections in the tree while the preview is down and hides only unsupported ones", async () => {
 const w = await setup({ listContentCollections: async () => [
  { name: "blog", kind: "markdown", editable: false, supportedForEditing: true, reason: "Start the preview to edit content" },
  { name: "legacy", kind: "markdown", editable: false, supportedForEditing: false, reason: "Use orsoGlob() to make this collection editable" },
 ] }); await flushPromises();
 const names = w.findAll('[data-testid="content-collection"]').map(c => c.text());
 expect(names).toContain("blog"); expect(names).not.toContain("legacy");
 w.unmount();
});
it("explains an empty catalog: the site needs the orso-astro integration", async () => {
 const w = await setup({ listContentCollections: async () => [] }); await flushPromises();
 const empty = w.get('[data-testid="content-empty-catalog"]').text();
 for (const part of ["orso-astro", "orso()", "astro.config", "orsoGlob", "src/content.config.ts", "preview starts"]) expect(empty).toContain(part);
 expect(w.text()).not.toContain("Select a collection and entry");
 w.unmount();
 const listed = await setup(); await flushPromises();
 expect(listed.find('[data-testid="content-empty-catalog"]').exists()).toBe(false);
 listed.unmount();
});
it("clears the previous project's catalog while the next project loads", async () => {
 const w = await setup(); await flushPromises(); const store = useContentStore();
 const opening = store.open("other");
 expect(store.collections).toEqual([]);
 await opening; w.unmount();
});
