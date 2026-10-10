// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createRouter, createMemoryHistory, RouterView } from "vue-router";
import { mount, flushPromises, type VueWrapper } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import { ApiError } from "@/core/api";
const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
import ContentPanel from "./ContentPanel.vue";
import { useContentStore } from "./store";

const blog = { name: "blog", kind: "markdown", editable: true, reason: null, fields: { type: "object", required: true, properties: {
  title: { type: "string", required: true },
} } };
const site = { ...blog, name: "site", kind: "json", singleton: "site" };
const entry = (over = {}) => ({ collection: "blog", id: "welcome", path: "src/content/blog/en/welcome.md", kind: "markdown", source: "S", fields: { title: "Welcome" }, body: "Body", branch: "content/a", hash: "h1", ...over });
const impact = {
  references: [{ collection: "blog", id: "launch", path: "src/content/blog/en/launch.md", field: "relatedPosts" }],
  assets: ["src/assets/blog/welcome/cover.png"],
  proseLinks: [{ collection: "blog", id: "launch", path: "src/content/blog/en/launch.md", url: "/blog/welcome", line: 8 }],
};
const unreferenced = { references: [], assets: ["src/assets/blog/welcome/cover.png"], proseLinks: [] };
let w: VueWrapper;
const mounted: VueWrapper[] = [];
async function setup(api: Record<string, unknown> = {}, collection: object = blog, loaded = entry()) {
  h.daemon = createFakeDaemon({
    listContentCollections: async () => [collection], listContentEntries: async () => [loaded],
    contentEntry: async () => loaded,
    contentImpact: vi.fn(async () => impact),
    renameContentEntry: vi.fn(async () => ({ entry: entry({ id: "hello", path: "src/content/blog/en/hello.md", hash: "h2" }), entries: [entry({ id: "hello" })] })),
    deleteContentEntry: vi.fn(async () => ({ entries: [] })),
    ...api,
  });
  setActivePinia(createPinia());
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: "/", component: ContentPanel, props: { projectId: "p" } }] });
  await router.push("/"); await router.isReady();
  w = mount(RouterView, { global: { plugins: [router] }, attachTo: document.body }); mounted.push(w);
  await flushPromises();
  await w.get('[data-testid="content-collection"]').trigger("click"); await flushPromises();
  await w.get('[data-testid="content-entry"]').trigger("click"); await flushPromises();
  return w;
}
beforeEach(() => { document.body.innerHTML = ""; });
afterEach(() => { mounted.splice(0).forEach(x => x.unmount()); });
const api = () => h.daemon.api as unknown as Record<"contentImpact" | "renameContentEntry" | "deleteContentEntry", ReturnType<typeof vi.fn>>;
const dialog = () => document.body.querySelector('[data-testid="content-lifecycle"]') as HTMLElement | null;
const el = <T extends HTMLElement = HTMLElement>(id: string) => document.body.querySelector(`[data-testid="${id}"]`) as T | null;
async function typeSlug(value: string) {
  const input = el<HTMLInputElement>("content-rename-slug")!;
  input.value = value; input.dispatchEvent(new Event("input")); await flushPromises();
}

it("shows references, assets, and prose links before a rename is confirmed", async () => {
  await setup();
  await w.get('[data-testid="content-rename"]').trigger("click"); await flushPromises();
  expect(api().contentImpact).toHaveBeenCalledWith("p", "blog", "welcome");
  const text = dialog()!.textContent!;
  expect(text).toContain("blog/launch");
  expect(text).toContain("relatedPosts");
  expect(text).toContain("src/assets/blog/welcome/cover.png");
  expect(text).toContain("/blog/welcome");
  expect(text).toContain("src/content/blog/en/launch.md:8");
  expect(api().renameContentEntry).not.toHaveBeenCalled();
  await typeSlug("hello");
  el("content-lifecycle-confirm")!.click(); await flushPromises();
  expect(api().renameContentEntry).toHaveBeenCalledWith({ project: "p", collection: "blog", id: "welcome", slug: "hello", branch: "content/a", hash: "h1" });
  const store = useContentStore();
  expect(store.entry?.id).toBe("hello");
  expect(store.entries.map(e => e.id)).toEqual(["hello"]);
  expect(dialog()).toBeNull();
});

it("refuses to confirm deleting a referenced entry and deletes an unreferenced one", async () => {
  await setup();
  await w.get('[data-testid="content-delete"]').trigger("click"); await flushPromises();
  expect(dialog()!.textContent).toContain("blog/launch");
  expect(el<HTMLButtonElement>("content-lifecycle-confirm")!.disabled).toBe(true);
  el("content-lifecycle-cancel")!.click(); await flushPromises();
  expect(api().deleteContentEntry).not.toHaveBeenCalled();

  api().contentImpact.mockResolvedValue(unreferenced);
  await w.get('[data-testid="content-delete"]').trigger("click"); await flushPromises();
  expect(dialog()!.textContent).toContain("src/assets/blog/welcome/cover.png");
  el("content-lifecycle-confirm")!.click(); await flushPromises();
  expect(api().deleteContentEntry).toHaveBeenCalledWith({ project: "p", collection: "blog", id: "welcome", branch: "content/a", hash: "h1" });
  const store = useContentStore();
  expect(store.entry).toBeNull();
  expect(store.entries).toEqual([]);
});

it("keeps the entry and dialog on a rename conflict", async () => {
  const conflict = new ApiError("CONFLICT", "The entry changed since it was opened; reload to compare", 409, { reason: "stale_content" });
  await setup({ renameContentEntry: vi.fn(async () => { throw conflict; }) });
  await w.get('[data-testid="content-rename"]').trigger("click"); await flushPromises();
  await typeSlug("hello");
  el("content-lifecycle-confirm")!.click(); await flushPromises();
  const store = useContentStore();
  expect(store.conflict?.reason).toBe("stale_content");
  expect(store.entry?.id).toBe("welcome");
  expect(w.get('[data-testid="content-conflict"]').text()).toContain("Reload entry");
  expect(dialog()!.textContent).toContain("The entry changed since it was opened");
  expect(el<HTMLInputElement>("content-rename-slug")!.value).toBe("hello");
});

it("keeps the entry and dialog on a delete refusal", async () => {
  const refused = new ApiError("CONFLICT", "blog/welcome is still referenced; remove those references first", 409, { reason: "entry_referenced" });
  await setup({ contentImpact: vi.fn(async () => unreferenced), deleteContentEntry: vi.fn(async () => { throw refused; }) });
  await w.get('[data-testid="content-delete"]').trigger("click"); await flushPromises();
  el("content-lifecycle-confirm")!.click(); await flushPromises();
  expect(dialog()!.textContent).toContain("still referenced");
  expect(useContentStore().entry?.id).toBe("welcome");
});

it("blocks rename and delete while the draft has unsaved edits", async () => {
  await setup();
  await w.get('[data-testid="field-title"]').setValue("Mine");
  expect(w.get('[data-testid="content-rename"]').attributes("disabled")).toBeDefined();
  expect(w.get('[data-testid="content-delete"]').attributes("disabled")).toBeDefined();
  expect(w.get('[data-testid="content-lifecycle-blocked"]').text()).toBe("Save or discard your changes first");
  const store = useContentStore();
  await store.startLifecycle("rename"); await store.startLifecycle("delete"); await flushPromises();
  expect(store.lifecycle).toBeNull();
  expect(dialog()).toBeNull();
  expect(api().contentImpact).not.toHaveBeenCalled();
  expect((w.get('[data-testid="field-title"]').element as HTMLInputElement).value).toBe("Mine");
  // Reverting the edit re-enables both actions.
  await w.get('[data-testid="field-title"]').setValue("Welcome");
  expect(w.get('[data-testid="content-rename"]').attributes("disabled")).toBeUndefined();
  expect(w.find('[data-testid="content-lifecycle-blocked"]').exists()).toBe(false);
});

it("resumes a rename on the new change after Create Change", async () => {
  const onMain = new ApiError("CHANGE_REQUIRED", "Start a change first", 409);
  const rename = vi.fn().mockRejectedValueOnce(onMain).mockResolvedValue({ entry: entry({ id: "hello", branch: "content/b" }), entries: [entry({ id: "hello" })] });
  await setup({ renameContentEntry: rename }, blog, entry({ branch: "main" }));
  await w.get('[data-testid="content-rename"]').trigger("click"); await flushPromises();
  await typeSlug("hello");
  el("content-lifecycle-confirm")!.click(); await flushPromises();
  const store = useContentStore();
  expect(store.needsChange).toBe(true);
  await store.resumeSave("content/b"); await flushPromises();
  expect(rename).toHaveBeenLastCalledWith(expect.objectContaining({ id: "welcome", slug: "hello", branch: "content/b" }));
  expect(store.entry?.id).toBe("hello");
});

it("offers no rename or delete for the site singleton or the home page", async () => {
  await setup({}, site, entry({ collection: "site", id: "site", kind: "json", body: "" }));
  expect(w.find('[data-testid="content-rename"]').exists()).toBe(false);
  expect(w.find('[data-testid="content-delete"]').exists()).toBe(false);
  mounted.splice(0).forEach(x => x.unmount());
  await setup({}, { ...blog, name: "pages", kind: "json" }, entry({ collection: "pages", id: "home", kind: "json", body: "" }));
  expect(w.find('[data-testid="content-rename"]').exists()).toBe(false);
  expect(w.find('[data-testid="content-delete"]').exists()).toBe(false);
});
