// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createRouter, createMemoryHistory, RouterView } from "vue-router";
import { mount, flushPromises, type VueWrapper } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import { ApiError } from "@/core/api";
import { EditorView } from "@codemirror/view";
const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
import ContentPanel from "./ContentPanel.vue";
import { useContentStore } from "./store";

const collection = { name: "blog", kind: "markdown", editable: true, reason: null, fields: { type: "object", required: true, properties: {
  title: { type: "string", required: true }, subtitle: { type: "string", required: false },
} } };
const entry = (over = {}) => ({ collection: "blog", id: "welcome", path: "src/content/blog/welcome.md", kind: "markdown", source: "S", fields: { title: "Welcome", subtitle: "Sub" }, body: "Body", branch: "content/a", hash: "h1", ...over });
let w: VueWrapper;
const mounted: VueWrapper[] = [];
async function setup(api: Record<string, unknown> = {}, loaded = entry()) {
  h.daemon = createFakeDaemon({
    listContentCollections: async () => [collection], listContentEntries: async () => [loaded],
    contentEntry: async () => loaded, updateContentEntry: vi.fn(async () => entry({ hash: "h2" })), ...api,
  });
  setActivePinia(createPinia());
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: "/", component: ContentPanel, props: { projectId: "p" } }] });
  await router.push("/"); await router.isReady();
  w = mount(RouterView, { global: { plugins: [router] }, attachTo: document.body }); mounted.push(w);
  await flushPromises();
  await w.get('[data-testid="content-collection"]').trigger("click"); await flushPromises();
  await w.get('[data-testid="content-entry"]').trigger("click"); await flushPromises();
  // The body editor is an async chunk.
  if (loaded.kind !== "json") await vi.waitFor(() => w.get('[data-testid="body-editor"]'));
  await flushPromises();
  return w;
}
beforeEach(() => { document.body.innerHTML = ""; });
afterEach(() => { mounted.splice(0).forEach(x => x.unmount()); });
/** Replace the body through the editor's Raw Markdown view, as a writer would. */
async function typeBody(text: string) {
  if (w.find('[data-testid="body-mode-raw"]').exists()) { await w.get('[data-testid="body-mode-raw"]').trigger("click"); await flushPromises(); }
  const view = EditorView.findFromDOM(w.get('[data-testid="body-raw"] .cm-editor').element as HTMLElement)!;
  view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text }, userEvent: "input.type" });
  await flushPromises();
}
const update = () => (h.daemon.api as unknown as { updateContentEntry: ReturnType<typeof vi.fn> }).updateContentEntry;

it("saves only on explicit Save, sending just the changed fields and the opened branch and hash", async () => {
  await setup();
  await w.get('[data-testid="field-title"]').setValue("Hello");
  expect(update()).not.toHaveBeenCalled();
  await w.get('[data-testid="content-save"]').trigger("submit"); await flushPromises();
  expect(update()).toHaveBeenCalledWith({ project: "p", collection: "blog", id: "welcome", branch: "content/a", hash: "h1", fields: { title: "Hello" } });
  expect(useContentStore().dirty).toBe(false);
  expect(useContentStore().entry?.hash).toBe("h2");
});

it("sends cleared optional fields in unset and a changed body", async () => {
  await setup();
  await w.get('[data-testid="field-subtitle"]').setValue("");
  await typeBody("New body");
  await w.get('[data-testid="content-save"]').trigger("submit"); await flushPromises();
  expect(update()).toHaveBeenCalledWith(expect.objectContaining({ fields: {}, unset: ["subtitle"], body: "New body" }));
});

it("makes no mutation call when nothing changed, including after reverting an edit", async () => {
  await setup();
  await w.get('[data-testid="field-title"]').setValue("X");
  await w.get('[data-testid="field-title"]').setValue("Welcome");
  expect(w.get('[data-testid="content-save"]').attributes("disabled")).toBeDefined();
  await expect(useContentStore().saveDraft()).resolves.toMatchObject({ id: "welcome" });
  expect(update()).not.toHaveBeenCalled();
});

it("shows validation helper paths and keeps the draft", async () => {
  const bad = new ApiError("BADREQ", "invalid", 400, { reason: "invalid_schema", details: [{ path: ["title"], message: "Too short" }] });
  await setup({ updateContentEntry: vi.fn(async () => { throw bad; }) });
  await w.get('[data-testid="field-title"]').setValue("x");
  await w.get('[data-testid="content-save"]').trigger("submit"); await flushPromises();
  expect(w.get('[data-testid="field-title-error"]').text()).toBe("title: Too short");
  expect(w.get('[data-testid="content-field-errors"]').text()).toContain("title: Too short");
  expect((w.get('[data-testid="field-title"]').element as HTMLInputElement).value).toBe("x");
});

it("keeps the draft on a branch change and offers reload and compare", async () => {
  const conflict = new ApiError("CONFLICT", "The project is now on content/b; the entry was opened on content/a", 409, { reason: "branch_changed" });
  const disk = entry({ branch: "content/b", hash: "h9", fields: { title: "Disk", subtitle: "Sub" } });
  let reads = 0;
  await setup({ updateContentEntry: vi.fn(async () => { throw conflict; }), contentEntry: async () => (reads++ ? disk : entry()) });
  await w.get('[data-testid="field-title"]').setValue("Mine");
  await w.get('[data-testid="content-save"]').trigger("submit"); await flushPromises();
  expect(w.get('[data-testid="content-conflict"]').text()).toContain("Your edits are kept");
  expect((w.get('[data-testid="field-title"]').element as HTMLInputElement).value).toBe("Mine");
  await w.get('[data-testid="content-compare"]').trigger("click"); await flushPromises();
  expect(w.get('[data-testid="content-compare-table"]').text()).toMatch(/title\s*Mine\s*Disk/);
  expect((w.get('[data-testid="field-title"]').element as HTMLInputElement).value).toBe("Mine");
  await w.get('[data-testid="content-reload"]').trigger("click"); await flushPromises();
  expect((w.get('[data-testid="field-title"]').element as HTMLInputElement).value).toBe("Disk");
  expect(useContentStore().entry?.branch).toBe("content/b");
  expect(w.find('[data-testid="content-conflict"]').exists()).toBe(false);
});

it("treats a stale hash as a conflict that retains input", async () => {
  const stale = new ApiError("CONFLICT", "The entry changed since it was opened; reload to compare", 409, { reason: "stale_content" });
  await setup({ updateContentEntry: vi.fn(async () => { throw stale; }) });
  await w.get('[data-testid="field-title"]').setValue("Mine");
  await w.get('[data-testid="content-save"]').trigger("submit"); await flushPromises();
  expect(w.find('[data-testid="content-compare"]').exists()).toBe(true);
  expect(useContentStore().draftFields.title).toBe("Mine");
});

it("offers Create Change on main and resumes the pending save on the new branch", async () => {
  const needs = new ApiError("CHANGE_REQUIRED", "Start a change first", 409);
  const saved = vi.fn()
    .mockRejectedValueOnce(needs)
    .mockResolvedValueOnce(entry({ branch: "content/new", hash: "h2" }));
  const createChange = vi.fn(async (_p: string, title: string) => ({ branch: "content/new", title }));
  await setup({ updateContentEntry: saved, createChange, listChanges: async () => [], activeChange: async () => null }, entry({ branch: "main" }));
  await w.get('[data-testid="field-title"]').setValue("Mine");
  await w.get('[data-testid="content-save"]').trigger("submit"); await flushPromises();
  expect(w.get('[data-testid="content-needs-change"]').text()).toContain("Create a change");
  await w.get('[data-testid="content-create-change"]').trigger("click"); await flushPromises();
  const input = document.body.querySelector("[role=dialog] input") as HTMLInputElement;
  input.value = "My change"; input.dispatchEvent(new Event("input", { bubbles: true })); await flushPromises();
  (document.body.querySelector("[role=dialog] button[type=submit]") as HTMLButtonElement).click(); await flushPromises();
  expect(createChange).toHaveBeenCalledWith("p", "My change");
  expect(saved).toHaveBeenCalledTimes(2);
  expect(saved.mock.calls[1][0]).toMatchObject({ branch: "content/new", hash: "h1", fields: { title: "Mine" } });
  expect(useContentStore().dirty).toBe(false);
  expect(w.find('[data-testid="content-needs-change"]').exists()).toBe(false);
});

it("keeps edits typed while a save is in flight", async () => {
  let release!: (v: unknown) => void;
  const hold = new Promise(r => { release = r; });
  await setup({ updateContentEntry: vi.fn(async () => { await hold; return entry({ hash: "h2", fields: { title: "Hello", subtitle: "Sub" } }); }) });
  await w.get('[data-testid="field-title"]').setValue("Hello");
  await w.get('[data-testid="content-save"]').trigger("submit");
  await w.get('[data-testid="field-subtitle"]').setValue("Typed during save");
  release(null); await flushPromises();
  const store = useContentStore();
  expect(store.entry?.hash).toBe("h2");
  expect(store.draftFields.subtitle).toBe("Typed during save");
  expect(store.dirty).toBe(true);
  expect(update()).toHaveBeenCalledTimes(1);
});

it("keeps the draft and reports the new branch when the resumed save conflicts", async () => {
  const stale = new ApiError("CONFLICT", "The entry changed since it was opened; reload to compare", 409, { reason: "stale_content" });
  const saved = vi.fn().mockRejectedValueOnce(new ApiError("CHANGE_REQUIRED", "Start a change first", 409)).mockRejectedValueOnce(stale);
  const createChange = vi.fn(async (_p: string, title: string) => ({ branch: "content/new", title }));
  const disk = entry({ branch: "content/new", hash: "h9", fields: { title: "Disk", subtitle: "Sub" } });
  let reads = 0;
  await setup({ updateContentEntry: saved, createChange, contentEntry: async () => (reads++ ? disk : entry({ branch: "main" })) }, entry({ branch: "main" }));
  await w.get('[data-testid="field-title"]').setValue("Mine");
  await w.get('[data-testid="content-save"]').trigger("submit"); await flushPromises();
  await w.get('[data-testid="content-create-change"]').trigger("click"); await flushPromises();
  const input = document.body.querySelector("[role=dialog] input") as HTMLInputElement;
  input.value = "C"; input.dispatchEvent(new Event("input", { bubbles: true })); await flushPromises();
  (document.body.querySelector("[role=dialog] button[type=submit]") as HTMLButtonElement).click(); await flushPromises();
  expect(w.find('[data-testid="content-needs-change"]').exists()).toBe(false);
  expect(w.get('[data-testid="content-conflict"]').text()).toContain("content/new");
  expect(useContentStore().draftFields.title).toBe("Mine");
  await w.get('[data-testid="content-compare"]').trigger("click"); await flushPromises();
  expect(w.get('[data-testid="content-compare-table"]').text()).toMatch(/title\s*Mine\s*Disk/);
  await w.get('[data-testid="content-reload"]').trigger("click"); await flushPromises();
  expect(useContentStore().entry?.branch).toBe("content/new");
});

it("opens and toggles a Blog body without dirtying the draft, then saves a body edit", async () => {
  await setup({}, entry({ body: "Intro\n\n* one\n" }));
  expect(w.find('[data-testid="body-rich"]').exists()).toBe(true);
  await w.get('[data-testid="body-mode-raw"]').trigger("click"); await flushPromises();
  await w.get('[data-testid="body-mode-rich"]').trigger("click"); await flushPromises();
  expect(useContentStore().dirty).toBe(false);
  expect(w.get('[data-testid="content-rename"]').attributes("disabled")).toBeUndefined();
  await typeBody("Intro\n\n* one\n* two\n");
  expect(useContentStore().dirty).toBe(true);
  await w.get('[data-testid="content-save"]').trigger("submit"); await flushPromises();
  expect(update()).toHaveBeenCalledWith(expect.objectContaining({ fields: {}, body: "Intro\n\n* one\n* two\n" }));
});

it("keeps section MDX bodies raw-only", async () => {
  const sections = { ...collection, name: "sections", kind: "mdx" };
  await setup({ listContentCollections: async () => [sections] }, entry({ collection: "sections", id: "hero", kind: "mdx", body: "Plain text\n" }));
  expect(w.find('[data-testid="body-mode-rich"]').exists()).toBe(false);
  expect(w.find('[data-testid="body-raw"]').exists()).toBe(true);
  expect(useContentStore().dirty).toBe(false);
});
