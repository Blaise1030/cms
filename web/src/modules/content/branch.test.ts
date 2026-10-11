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
const entry = (over = {}) => ({ collection: "blog", id: "welcome", path: "src/content/blog/en/welcome.md", kind: "markdown", source: "S", fields: { title: "Welcome" }, body: "Body", branch: "content/a", hash: "h1", ...over });
const missing = () => new ApiError("ENOENT", "no such content entry", 404);
const branchChanged = () => new ApiError("CONFLICT", "The project is now on content/b; the entry was opened on content/a", 409, { reason: "branch_changed" });

/** A store with blog/welcome opened on content/a; `onDisk` answers reads on other branches. */
async function opened(onDisk: (branch: string) => unknown) {
  let branch = "content/a";
  const contentEntry = vi.fn(async () => branch === "content/a" ? entry() : onDisk(branch));
  // Like the daemon: a save must name the branch the project is on.
  const updateContentEntry = vi.fn(async (body: { branch: string }) => {
    if (body.branch !== branch) throw branchChanged();
    return { ...entry({ hash: "h2", source: "S2" }), fields: { title: "Edited" } };
  });
  h.daemon = createFakeDaemon({
    listContentCollections: async () => [blog], listContentEntries: async () => [entry()],
    contentEntry, updateContentEntry,
  });
  setActivePinia(createPinia());
  const store = useContentStore();
  await store.open("p");
  await store.selectCollection("blog");
  await store.selectEntry("welcome");
  return { store, updateContentEntry, switchTo: (next: string) => { branch = next; } };
}

beforeEach(() => { vi.restoreAllMocks(); });

it("keeps a dirty draft on the branch it was opened on, even when the file is identical elsewhere", async () => {
  const { store, switchTo, updateContentEntry } = await opened(b => entry({ branch: b }));
  store.draftFields.title = "Edited";
  const draft = store.draftFields;
  switchTo("content/b");
  await store.refreshForBranch("content/b");
  expect(store.draftFields).toBe(draft);
  expect(store.entry?.branch).toBe("content/a");
  expect(store.entry?.hash).toBe("h1");
  expect(store.changedOnDisk).toBe(true);
  await expect(store.saveDraft()).rejects.toBeInstanceOf(ApiError);
  expect(updateContentEntry).toHaveBeenCalledTimes(1);
  expect(updateContentEntry).toHaveBeenCalledWith(expect.objectContaining({ branch: "content/a", hash: "h1" }));
  expect(store.conflict?.reason).toBe("branch_changed");
  expect(store.draftFields).toBe(draft);
  expect(store.draftFields.title).toBe("Edited");
});

it("refuses to save a draft after switching to a branch with a different version, without a second write", async () => {
  const { store, switchTo, updateContentEntry } = await opened(
    b => entry({ branch: b, hash: "h9", source: "T", fields: { title: "Other" } }),
  );
  store.draftFields.title = "Edited";
  const draft = store.draftFields;
  switchTo("content/b");
  await store.refreshForBranch("content/b");
  expect(store.changedOnDisk).toBe(true);
  expect(store.entry?.branch).toBe("content/a");
  await expect(store.saveDraft()).rejects.toBeInstanceOf(ApiError);
  expect(updateContentEntry).toHaveBeenCalledTimes(1);
  expect(store.conflict?.reason).toBe("branch_changed");
  expect(store.draftFields).toBe(draft);
  expect(store.draftFields.title).toBe("Edited");
  expect(store.dirty).toBe(true);
});

it("clears the flag and saves the kept draft after switching back to its branch", async () => {
  const { store, switchTo, updateContentEntry } = await opened(b => entry({ branch: b, hash: "h9" }));
  store.draftFields.title = "Edited";
  const draft = store.draftFields;
  switchTo("content/b");
  await store.refreshForBranch("content/b");
  expect(store.changedOnDisk).toBe(true);
  switchTo("content/a");
  await store.refreshForBranch("content/a");
  expect(store.changedOnDisk).toBe(false);
  expect(store.draftFields).toBe(draft);
  await store.saveDraft();
  expect(updateContentEntry).toHaveBeenCalledTimes(1);
  expect(updateContentEntry).toHaveBeenCalledWith(expect.objectContaining({ branch: "content/a", hash: "h1", fields: { title: "Edited" } }));
  expect(store.conflict).toBeNull();
  expect(store.dirty).toBe(false);
});

it("keeps a draft whose entry does not exist on the new branch", async () => {
  const { store, switchTo } = await opened(() => { throw missing(); });
  store.draftFields.title = "Edited";
  switchTo("content/b");
  await store.refreshForBranch("content/b");
  expect(store.entry?.id).toBe("welcome");
  expect(store.draftFields.title).toBe("Edited");
  expect(store.changedOnDisk).toBe(true);
  expect(store.error).toBeNull();
});

it("reloads a clean entry from the new branch", async () => {
  const { store, switchTo } = await opened(b => entry({ branch: b, hash: "h2", fields: { title: "On b" } }));
  switchTo("content/b");
  await store.refreshForBranch("content/b");
  expect(store.entry?.branch).toBe("content/b");
  expect(store.draftFields.title).toBe("On b");
  expect(store.dirty).toBe(false);
});

it("closes a clean entry that does not exist on the new branch and lists the branch's entries", async () => {
  const { store, switchTo } = await opened(() => { throw missing(); });
  switchTo("main");
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
