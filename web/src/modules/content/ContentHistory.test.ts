// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { RouterView } from "vue-router";
import { contentRouter } from "@/test/contentRouter";
import { mount, flushPromises, type VueWrapper } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("./ContentTree.vue", () => import("@/test/contentTreeStub"));
vi.mock("@/modules/checkpoints/DiffView.vue", () => ({ __esModule: true, default: { template: "<div data-testid='diff-view' />" } }));
vi.mock("./MarkdownBodyEditor.vue", () => ({ __esModule: true, default: { props: ["body"], template: "<div data-testid='body-editor'>{{ body }}</div>" } }));
import { REFRESH_DELAY_MS } from "./store";

const blog = { name: "blog", kind: "markdown", editable: true, reason: null, fields: { type: "object", required: true, properties: {
  title: { type: "string", required: true },
} } };
const site = { name: "site", kind: "json", editable: true, reason: null, singleton: "site", fields: { type: "object", required: true, properties: {
  title: { type: "string", required: true },
} } };
const entry = (over = {}) => ({ collection: "blog", id: "welcome", path: "src/content/blog/en/welcome.md", kind: "markdown", source: "---\ntitle: Welcome\n---\nBody", fields: { title: "Welcome" }, body: "Body", branch: "content/a", hash: "h1", ...over });
const versions = [
  { sha: "s2", summary: "Edit", author: "Ann", ts: 1_760_000_000, path: "src/content/blog/en/welcome.md", published: false },
  { sha: "s1", summary: "Create", author: "Ann", ts: 1_759_000_000, path: "src/content/blog/en/welcome.md", published: true },
];
const old = { sha: "s1", path: "src/content/blog/en/welcome.md", source: "---\ntitle: Old\n---\nOld body", fields: { title: "Old" }, body: "Old body", parseable: true };

let w: VueWrapper;
beforeEach(() => { document.body.innerHTML = ""; vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] }); });
afterEach(() => { w?.unmount(); vi.useRealTimers(); });

async function setup(api: Record<string, unknown> = {}, opts: { collection?: object; loaded?: ReturnType<typeof entry> } = {}) {
  const loaded = opts.loaded ?? entry();
  const api2 = {
    listContentCollections: async () => [opts.collection ?? blog],
    listContentEntries: async () => [loaded, entry({ id: "second" })],
    contentEntry: async (_p: string, _c: string, id: string) => id === loaded.id ? loaded : entry({ id: "second", hash: "h9" }),
    contentHistory: vi.fn(async () => versions),
    contentVersion: vi.fn(async () => old),
    updateContentEntry: vi.fn(),
    ...api,
  };
  h.daemon = createFakeDaemon(api2);
  setActivePinia(createPinia());
  const router = await contentRouter(`/p/p/content/${loaded.collection}/${loaded.id}`);
  w = mount(RouterView, { global: { plugins: [router] }, attachTo: document.body });
  await flushPromises();
  return api2;
}
const tab = (name: string) => w.findAll('[role="tab"]').find(t => t.text() === name);
async function openHistory() { await tab("History")!.trigger("mousedown", { button: 0 }); await flushPromises(); }
async function selectOld() { await w.findAll('[data-testid="history-version"]')[1].trigger("click"); await flushPromises(); }
const button = (text: string) => [...document.querySelectorAll("button")].find(b => b.textContent?.trim() === text) as HTMLButtonElement | undefined;
const title = () => w.get<HTMLInputElement>('[data-testid="field-title"]');
async function typeTitle(value: string) {
  await tab("Fields")!.trigger("mousedown", { button: 0 }); await flushPromises();
  await title().setValue(value); await flushPromises();
}

it("History tab lists versions", async () => {
  const api = await setup();
  expect(api.contentHistory).not.toHaveBeenCalled();
  await openHistory();
  expect(api.contentHistory).toHaveBeenCalledWith("p", "blog", "welcome");
  expect(w.findAll('[data-testid="history-version"]')).toHaveLength(2);
});

it("selecting a version swaps body for diff", async () => {
  await setup();
  expect(w.find('[data-testid="body-editor"]').exists()).toBe(true);
  await openHistory(); await selectOld();
  expect(w.find('[data-testid="body-editor"]').exists()).toBe(false);
  expect(w.find('[data-testid="version-diff"]').exists()).toBe(true);
  button("Back to editor")!.click(); await flushPromises();
  expect(w.find('[data-testid="version-diff"]').exists()).toBe(false);
  expect(w.find('[data-testid="body-editor"]').exists()).toBe(true);
});

it("Escape returns from the diff to the editor", async () => {
  await setup();
  await openHistory(); await selectOld();
  await w.get('[data-testid="version-diff"]').trigger("keydown", { key: "Escape" }); await flushPromises();
  expect(w.find('[data-testid="version-diff"]').exists()).toBe(false);
  expect(w.find('[data-testid="body-editor"]').exists()).toBe(true);
});

it("restore fills the draft and returns to editor", async () => {
  const api = await setup();
  await openHistory(); await selectOld();
  button("Restore this version")!.click(); await flushPromises();
  expect(w.find('[data-testid="version-diff"]').exists()).toBe(false);
  expect(w.find('[data-testid="body-editor"]').exists()).toBe(true);
  await tab("Fields")!.trigger("mousedown", { button: 0 }); await flushPromises();
  expect(title().element.value).toBe("Old");
  expect(w.get<HTMLButtonElement>('[data-testid="content-save"]').element.disabled).toBe(false);
  expect(api.updateContentEntry).not.toHaveBeenCalled();
});

it("restore over unsaved edits asks first", async () => {
  await setup();
  await typeTitle("Mine");
  await openHistory(); await selectOld();
  button("Restore this version")!.click(); await flushPromises();
  expect(document.body.textContent).toContain("Replace unsaved edits?");
  button("Cancel")!.click(); await flushPromises();
  expect(document.body.textContent).not.toContain("Replace unsaved edits?");
  await tab("Fields")!.trigger("mousedown", { button: 0 }); await flushPromises();
  expect(title().element.value).toBe("Mine");
});

it("confirming the replace restores over unsaved edits", async () => {
  await setup();
  await typeTitle("Mine");
  await openHistory(); await selectOld();
  button("Restore this version")!.click(); await flushPromises();
  button("Replace")!.click(); await flushPromises();
  expect(w.find('[data-testid="version-diff"]').exists()).toBe(false);
  await tab("Fields")!.trigger("mousedown", { button: 0 }); await flushPromises();
  expect(title().element.value).toBe("Old");
});

it("navigating to another entry clears the selected version", async () => {
  await setup();
  await openHistory(); await selectOld();
  await w.vm.$router.push("/p/p/content/blog/second"); await flushPromises();
  expect(w.find('[data-testid="version-diff"]').exists()).toBe(false);
  expect(w.find('[data-testid="body-editor"]').exists()).toBe(true);
});

it("unsaved edits show notice", async () => {
  await setup();
  await typeTitle("Mine");
  await openHistory(); await selectOld();
  expect(w.text()).toContain("Comparing with the last saved version");
});

it("JSON entry shows History tab", async () => {
  await setup({}, { collection: site, loaded: entry({ collection: "site", id: "site", kind: "json", path: "src/content/site.json", source: "{}", body: undefined }) });
  expect(tab("History")).toBeDefined();
  expect(tab("Fields")).toBeUndefined();
  expect(w.findAll('[data-testid="history-version"]')).toHaveLength(2);
});

it("restore is disabled on main", async () => {
  await setup({}, { loaded: entry({ branch: "main" }) });
  await openHistory(); await selectOld();
  expect(button("Restore this version")!.disabled).toBe(true);
  expect(w.text()).toContain("Start a change to restore this version.");
});

it("restore is disabled for a version that could not be parsed", async () => {
  await setup({ contentVersion: async () => ({ ...old, parseable: false }) });
  await openHistory(); await selectOld();
  expect(button("Restore this version")!.disabled).toBe(true);
  expect(w.text()).toContain("This version can't be restored because it couldn't be parsed.");
});

it("reloads history when the open entry's file moves", async () => {
  let loaded = entry();
  const api = await setup({ contentEntry: async () => loaded });
  await openHistory();
  expect(api.contentHistory).toHaveBeenCalledTimes(1);
  loaded = entry({ hash: "h2" });
  h.daemon.emit({ type: "fs.change", projectId: "p", path: loaded.path, kind: "modify" });
  vi.advanceTimersByTime(REFRESH_DELAY_MS); await flushPromises();
  expect(api.contentHistory).toHaveBeenCalledTimes(2);
});
