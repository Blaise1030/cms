// @vitest-environment happy-dom
import { beforeEach, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import { ApiError } from "@/core/api";
const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
import { useContentStore } from "./store";

const blog = { name: "blog", kind: "markdown", editable: true, reason: null, fields: { type: "object", required: true, properties: {
  title: { type: "string", required: true },
} } };
const entry = (over = {}) => ({ collection: "blog", id: "welcome", path: "src/content/blog/en/welcome.md", kind: "markdown", source: "S", fields: { title: "Welcome" }, body: "Body", branch: "main", hash: "h1", ...over });
const missing = () => new ApiError("ENOENT", "no such content entry", 404);

/** A store with blog/welcome opened on main; `onBranch` answers reads after the switch. */
async function opened(onBranch: () => unknown, listed = [entry()]) {
  let switched = false;
  const contentEntry = vi.fn(async () => { if (!switched) return entry(); return onBranch(); });
  const updateContentEntry = vi.fn(async () => ({ ...entry({ branch: "content/b", hash: "h2" }), fields: { title: "Edited" } }));
  h.daemon = createFakeDaemon({
    listContentCollections: async () => [blog], listContentEntries: async () => listed,
    contentEntry, updateContentEntry,
  });
  setActivePinia(createPinia());
  const store = useContentStore();
  await store.open("p");
  await store.selectCollection("blog");
  await store.selectEntry("welcome");
  return { store, contentEntry, updateContentEntry, switchBranch: () => { switched = true; } };
}

beforeEach(() => { vi.restoreAllMocks(); });

it("keeps the draft object and moves it to the new branch when the entry is unchanged there", async () => {
  const { store, switchBranch, updateContentEntry } = await opened(() => entry({ branch: "content/b" }));
  store.draftFields.title = "Edited";
  const draft = store.draftFields;
  switchBranch();
  await store.refreshForBranch("content/b");
  expect(store.draftFields).toBe(draft);
  expect(store.draftFields.title).toBe("Edited");
  expect(store.dirty).toBe(true);
  expect(store.entry?.branch).toBe("content/b");
  expect(store.changedOnDisk).toBe(false);
  await store.saveDraft();
  expect(updateContentEntry).toHaveBeenCalledWith(expect.objectContaining({ branch: "content/b", hash: "h1", fields: { title: "Edited" } }));
});

it("keeps the draft on its loaded branch and flags it when the entry differs on the new branch", async () => {
  const { store, switchBranch } = await opened(() => entry({ branch: "content/b", hash: "h9", source: "T", fields: { title: "Other" } }));
  store.draftFields.title = "Edited";
  const draft = store.draftFields;
  switchBranch();
  await store.refreshForBranch("content/b");
  expect(store.draftFields).toBe(draft);
  expect(store.draftFields.title).toBe("Edited");
  expect(store.entry?.branch).toBe("main");
  expect(store.entry?.hash).toBe("h1");
  expect(store.changedOnDisk).toBe(true);
});

it("keeps a draft whose entry does not exist on the new branch", async () => {
  const { store, switchBranch } = await opened(() => { throw missing(); });
  store.draftFields.title = "Edited";
  switchBranch();
  await store.refreshForBranch("content/b");
  expect(store.entry?.id).toBe("welcome");
  expect(store.draftFields.title).toBe("Edited");
  expect(store.changedOnDisk).toBe(true);
  expect(store.error).toBeNull();
});

it("reloads a clean entry from the new branch", async () => {
  const { store, switchBranch } = await opened(() => entry({ branch: "content/b", hash: "h2", fields: { title: "On b" } }));
  switchBranch();
  await store.refreshForBranch("content/b");
  expect(store.entry?.branch).toBe("content/b");
  expect(store.draftFields.title).toBe("On b");
  expect(store.dirty).toBe(false);
});

it("closes a clean entry that does not exist on the new branch and lists the branch's entries", async () => {
  const { store, switchBranch } = await opened(() => { throw missing(); });
  switchBranch();
  await store.refreshForBranch("main");
  expect(store.entry).toBeNull();
  expect(store.error).toBeNull();
  expect(store.selectedCollection).toBe("blog");
  expect(store.entries.map(e => e.id)).toEqual(["welcome"]);
});

it("does nothing before a project is open", async () => {
  const listContentCollections = vi.fn();
  h.daemon = createFakeDaemon({ listContentCollections });
  setActivePinia(createPinia());
  const store = useContentStore();
  await store.refreshForBranch("content/b");
  expect(listContentCollections).not.toHaveBeenCalled();
});
