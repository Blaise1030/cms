// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createRouter, createMemoryHistory, RouterView } from "vue-router";
import { mount, flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
import ContentPanel from "./ContentPanel.vue";
import { REFRESH_DELAY_MS, useContentStore } from "./store";
beforeEach(() => { vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] }); });
afterEach(() => { vi.useRealTimers(); });
/** Emit a file event and let its debounced refresh start. */
function fsChange(path: string) { h.daemon.emit({type:"fs.change",projectId:"p",path,kind:"modify"}); vi.advanceTimersByTime(REFRESH_DELAY_MS); }
const entry = { collection: "blog", id: "welcome", path: "src/content/blog/en/welcome.md", kind: "markdown", source: "Original", fields: {title: "Welcome"}, body: "Original", branch: "main", hash: "abc" };
async function setup(api: Record<string, unknown> = {}) {
 h.daemon = createFakeDaemon({ listContentCollections: async () => [{name:"blog",kind:"markdown",editable:true,reason:null}], listContentEntries: async () => [entry], contentEntry: async () => entry, ...api });
 setActivePinia(createPinia());
 const router = createRouter({history:createMemoryHistory(),routes:[{path:"/",component:ContentPanel,props:{projectId:"p"}},{path:"/other",component:{template:"<p>Other</p>"}}]});
 await router.push("/"); await router.isReady();
 return mount(RouterView, {global:{plugins:[router]}});
}
it("navigates collections and entries and reads content on main", async () => {
 const w = await setup(); await flushPromises();
 await w.get('[data-testid="content-collection"]').trigger("click"); await flushPromises();
 await w.get('[data-testid="content-entry"]').trigger("click"); await flushPromises();
 expect(w.text()).toContain("main"); expect(w.get('[data-testid="content-source"]').text()).toBe("Original");
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
 expect(w.vm.$route.path).toBe("/"); expect(store.draftSource).toBe("Draft");
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
it("clears the previous project's catalog while the next project loads", async () => {
 const w = await setup(); await flushPromises(); const store = useContentStore();
 const opening = store.open("other");
 expect(store.collections).toEqual([]);
 await opening; w.unmount();
});
