// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { RouterView } from "vue-router";
import { contentRouter } from "@/test/contentRouter";
import { mount, flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
import { REFRESH_DELAY_MS, useContentStore } from "./store";
beforeEach(() => { vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] }); });
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });
/** Emit a file event and let its debounced refresh start. */
function fsChange(path: string) { h.daemon.emit({type:"fs.change",projectId:"p",path,kind:"modify"}); vi.advanceTimersByTime(REFRESH_DELAY_MS); }
const welcome = { collection: "blog", id: "welcome", path: "src/content/blog/en/welcome.md", kind: "markdown", source: "Original", fields: {title: "Welcome"}, body: "Original", branch: "main", hash: "abc" };
const spring = { ...welcome, id: "spring-launch", path: "src/content/blog/en/spring-launch.md", source: "Spring source", hash: "s1" };
const blog = { name: "blog", kind: "markdown", editable: true, reason: null };
async function setup(api: Record<string, unknown> = {}, start = "/p/p/content") {
 h.daemon = createFakeDaemon({
  listContentCollections: async () => [blog],
  listContentEntries: async () => [welcome, spring],
  contentEntry: async (_p: string, _c: string, id: string) => id === "spring-launch" ? spring : welcome,
  ...api,
 });
 setActivePinia(createPinia());
 return mount(RouterView, {global:{plugins:[await contentRouter(start)]}});
}
it("lists collections, then entries, then opens the editor", async () => {
 const w = await setup(); await flushPromises();
 expect(w.find('[data-testid="content-entry-list"]').exists()).toBe(false);
 expect(w.text()).toContain("Choose a collection.");
 await w.get('[data-collection-name="blog"]').trigger("click"); await flushPromises();
 expect(w.vm.$route.path).toBe("/p/p/content/blog");
 expect(w.get('[data-testid="content-entry-list"]').text()).toContain("Blog");
 await w.get('[data-entry-id="welcome"]').trigger("click"); await flushPromises();
 expect(w.vm.$route.path).toBe("/p/p/content/blog/welcome");
 expect(w.get('[data-testid="content-entry-title"]').text()).toBe("Welcome");
 expect(w.find('[data-testid="content-collections"]').exists()).toBe(false);
 w.unmount();
});
it("returns to the list from the editor", async () => {
 const w = await setup({}, "/p/p/content/blog/welcome"); await flushPromises();
 await w.get('[data-testid="content-back"]').trigger("click"); await flushPromises();
 expect(w.vm.$route.path).toBe("/p/p/content/blog");
 expect(w.get('[data-testid="content-entry-list"]').text()).toContain("Welcome");
 w.unmount();
});
it("opens the collection and entry named in the URL", async () => {
 const contentEntry = vi.fn(async () => welcome);
 const w = await setup({ contentEntry }, "/p/p/content/blog/welcome"); await flushPromises();
 expect(contentEntry).toHaveBeenCalledWith("p", "blog", "welcome");
 expect(w.get('[data-testid="content-source"]').text()).toBe("Original");
 expect(w.get('[data-testid="content-entry-title"]').text()).toBe("Welcome");
 w.unmount();
});
it("opens a singleton's entry and returns to the collection index", async () => {
 const site = { collection: "site", id: "site", path: "src/content/site.json", kind: "json", source: "{}", fields: { title: "Site" }, branch: "main", hash: "h1" };
 const w = await setup({
  listContentCollections: async () => [{ name: "site", kind: "json", singleton: "site", editable: true, reason: null }],
  listContentEntries: async () => [site],
  contentEntry: async () => site,
 }, "/p/p/content/site"); await flushPromises();
 expect(w.vm.$route.path).toBe("/p/p/content/site/site");
 expect(w.get('[data-testid="content-entry-title"]').text()).toBe("Site");
 await w.get('[data-testid="content-back"]').trigger("click"); await flushPromises();
 expect(w.vm.$route.path).toBe("/p/p/content");
 w.unmount();
});
function deferred<T>() {
 let resolve!: (value: T) => void;
 const promise = new Promise<T>(done => { resolve = done; });
 return {promise, resolve};
}
it("shows the list for a deep link once the catalog arrives", async () => {
 const catalog = deferred<typeof blog[]>();
 const w = await setup({ listContentCollections: () => catalog.promise }, "/p/p/content/blog");
 await flushPromises();
 expect(w.findAll('[data-testid="content-entry"]')).toHaveLength(0);
 expect(w.text()).not.toContain("No content collections");
 catalog.resolve([blog]); await flushPromises();
 expect(w.findAll('[data-testid="content-entry"]')).toHaveLength(2);
 w.unmount();
});
it("refuses the back link while the draft is dirty", async () => {
 const w = await setup({}, "/p/p/content/blog/welcome"); await flushPromises();
 const store = useContentStore();
 store.draftSource = "Draft";
 vi.stubGlobal("confirm", vi.fn(() => false));
 await w.get('[data-testid="content-back"]').trigger("click"); await flushPromises();
 expect(w.vm.$route.path).toBe("/p/p/content/blog/welcome");
 expect(store.draftSource).toBe("Draft");
 w.unmount();
});
it("shows the neutral pane for a collection the sidebar hides", async () => {
 const legacy = { name: "legacy", kind: "markdown", editable: false, supportedForEditing: false, reason: "Use orsoGlob() to make this collection editable" };
 const w = await setup({ listContentCollections: async () => [blog, legacy] }, "/p/p/content/legacy"); await flushPromises();
 expect(w.findAll('[data-collection-name]').map(c => c.attributes("data-collection-name"))).toEqual(["blog"]);
 expect(w.text()).toContain("Choose a collection.");
 w.unmount();
});
it("offers create in an empty list and opens a read-only collection", async () => {
 const fields = { type: "object", required: true, properties: {} };
 const empty = await setup({ listContentCollections: async () => [{ ...blog, fields }], listContentEntries: async () => [] }); await flushPromises();
 await empty.get('[data-collection-name="blog"]').trigger("click"); await flushPromises();
 expect(empty.text()).toContain("No entries yet");
 expect(empty.get('[data-testid="content-new-entry"]').attributes("disabled")).toBeUndefined();
 empty.unmount();
 const readOnly = await setup({ listContentCollections: async () => [{ ...blog, fields, editable: false, supportedForEditing: true, reason: "Start the preview to edit content" }] }); await flushPromises();
 await readOnly.get('[data-collection-name="blog"]').trigger("click"); await flushPromises();
 await readOnly.get('[data-entry-id="welcome"]').trigger("click"); await flushPromises();
 expect(readOnly.vm.$route.path).toBe("/p/p/content/blog/welcome");
 expect(readOnly.get('[data-testid="content-read-only"]').text()).toBe("Start the preview to edit content");
 readOnly.unmount();
});
it("labels the collection list and filters it as you type", async () => {
 const help = { ...blog, name: "helpDocs" };
 const w = await setup({ listContentCollections: async () => [blog, help] }); await flushPromises();
 expect(w.text()).toContain("Collections");
 expect(w.get('[data-slot="input-group"]').find("svg.lucide-search").exists()).toBe(true);
 await w.get('[data-testid="content-collection-filter"]').setValue("help");
 expect(w.findAll("[data-collection-name]").map(c => c.attributes("data-collection-name"))).toEqual(["helpDocs"]);
 await w.get('[data-testid="content-collection-filter"]').setValue("zzz");
 expect(w.get('[data-testid="content-collections-empty"]').text()).toBe("No matches");
 w.unmount();
});

it("shows the document's slug and title fields together", async () => {
 const fields = { type: "object", required: true, properties: { title: { type: "string", required: true } } };
 const impact = { references: [], assets: [], proseLinks: [] };
 const w = await setup({
  listContentCollections: async () => [{ ...blog, fields }],
  contentImpact: async () => impact,
 }, "/p/p/content/blog/welcome"); await flushPromises();
 expect((w.get('[data-testid="content-slug"]').element as HTMLInputElement).value).toBe("welcome");
 expect(w.get('[data-testid="content-slug"]').attributes("required")).toBeDefined();
 expect(w.get('[data-testid="content-slug"]').attributes("aria-required")).toBe("true");
 expect(w.get('label[for="content-slug"]').text()).toBe("Slug*");
 expect(w.find('[data-testid="field-title"]').exists()).toBe(true);
 await w.get('[data-testid="content-slug"]').setValue("welcome-again");
 await w.get('[data-testid="content-slug-apply"]').trigger("click"); await flushPromises();
 const rename = document.body.querySelector<HTMLInputElement>('[data-testid="content-rename-slug"]')!;
 expect(rename.value).toBe("welcome-again");
 w.unmount();
});

it("leaves the slug to the schema when the collection declares one", async () => {
 const fields = { type: "object", required: true, properties: { title: { type: "string", required: true }, slug: { type: "string", required: true } } };
 const w = await setup({ listContentCollections: async () => [{ ...blog, fields }] }, "/p/p/content/blog/welcome"); await flushPromises();
 expect(w.find('[data-testid="content-slug-field"]').exists()).toBe(false);
 expect(w.find('[data-testid="field-slug"]').exists()).toBe(true);
 w.unmount();
});

it("restores the last collection's list when the tab reopens", async () => {
 const w = await setup({}, "/p/p/content/blog/welcome"); await flushPromises();
 await w.vm.$router.push("/other"); await w.vm.$router.push("/p/p/content"); await flushPromises();
 expect(w.vm.$route.path).toBe("/p/p/content/blog");
 w.unmount();
});
it("preserves a dirty draft on filesystem refresh and cancelled navigation", async () => {
 let onDisk = welcome;
 const w = await setup({ contentEntry: async () => onDisk }); await flushPromises(); const store = useContentStore();
 await store.selectCollection("blog"); await store.selectEntry("welcome"); store.draftSource = "Draft";
 // An unrelated file event does not flag the draft; a change to its file does.
 fsChange("notes.txt"); await flushPromises();
 expect(store.changedOnDisk).toBe(false);
 onDisk = {...welcome, source: "Changed", hash: "def"};
 fsChange(welcome.path); await flushPromises();
 expect(store.draftSource).toBe("Draft"); expect(store.dirty).toBe(true); expect(store.changedOnDisk).toBe(true);
 vi.stubGlobal("confirm", vi.fn(() => false)); await store.selectCollection("blog"); expect(store.entry?.id).toBe("welcome");
 w.unmount();
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
 w.unmount();
});
it.each(["selection-first", "refresh-first"])("keeps the user's entry selection and clears loading during %s overlap", async (ordering) => {
 const selection = deferred<typeof spring>();
 const catalog = deferred<{name:string;kind:string;editable:boolean;reason:null}[]>();
 let holdCatalog = false;
 const w = await setup({
  listContentCollections: async () => holdCatalog ? catalog.promise : [blog],
  contentEntry: async (_project: string, _collection: string, id: string) => id === "spring-launch" ? selection.promise : welcome,
 });
 await flushPromises(); const store = useContentStore();
 await store.selectCollection("blog"); await store.selectEntry("welcome");
 holdCatalog = true;
 let chosen: Promise<void>;
 if (ordering === "selection-first") {
  chosen = store.selectEntry("spring-launch");
  fsChange(welcome.path);
 } else {
  fsChange(welcome.path);
  chosen = store.selectEntry("spring-launch");
 }
 await flushPromises(); expect(store.loading).toBe(true);
 selection.resolve(spring); await chosen;
 catalog.resolve([blog]); await flushPromises();
 expect(store.entry?.id).toBe("spring-launch");
 expect(store.draftSource).toBe("Spring source");
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
it("keeps supported collections listed while the preview is down and hides only unsupported ones", async () => {
 const w = await setup({ listContentCollections: async () => [
  { name: "blog", kind: "markdown", editable: false, supportedForEditing: true, reason: "Start the preview to edit content" },
  { name: "legacy", kind: "markdown", editable: false, supportedForEditing: false, reason: "Use orsoGlob() to make this collection editable" },
 ] }); await flushPromises();
 const names = w.findAll('[data-testid="content-collection"]').map(c => c.text());
 expect(names).toContain("Blog"); expect(names).not.toContain("Legacy");
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
