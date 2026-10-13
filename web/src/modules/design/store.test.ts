// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import { ApiError } from "@/core/api";
import type { DesignDoc } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
import { useDesignStore } from "./store";

const doc = (over: Partial<DesignDoc> = {}): DesignDoc => ({
  exists: true, tokens: { components: { x: 1 }, colors: { primary: "#000" } }, body: "# Design", hash: "h1", branch: "content/a", ...over,
});
const missing = (): DesignDoc => ({
  exists: false, tokens: {}, body: "", branch: "main", starter: { tokens: { colors: { primary: "#111" } }, body: "# Starter" },
});

function setup(api: Record<string, unknown> = {}) {
  h.daemon = createFakeDaemon({ design: vi.fn(async () => doc()), saveDesign: vi.fn(async () => doc({ hash: "h2" })), ...api });
  setActivePinia(createPinia());
  const store = useDesignStore();
  const fns = h.daemon.api as unknown as Record<"design" | "saveDesign", ReturnType<typeof vi.fn>>;
  return { store, fns };
}
const checkpoint = { type: "checkpoint.created", projectId: "p", sha: "s", summary: "x", author: "a", ts: 1 } as const;

beforeEach(() => { vi.restoreAllMocks(); });

describe("design store", () => {
  it("open loads the doc and drafts", async () => {
    const { store, fns } = setup();
    await store.open("p");
    expect(fns.design).toHaveBeenCalledWith("p");
    expect(store.doc?.hash).toBe("h1");
    expect(store.draftTokens).toEqual({ components: { x: 1 }, colors: { primary: "#000" } });
    expect(store.draftBody).toBe("# Design");
    expect(store.dirty).toBe(false);
    store.setBody("# Edited");
    expect(store.dirty).toBe(true);
    store.setBody("# Design");
    expect(store.dirty).toBe(false);
  });

  it("create fills drafts from starter and is dirty", async () => {
    const { store } = setup({ design: vi.fn(async () => missing()) });
    await store.open("p");
    expect(store.doc?.exists).toBe(false);
    expect(store.dirty).toBe(false);
    store.create();
    expect(store.draftTokens).toEqual({ colors: { primary: "#111" } });
    expect(store.draftBody).toBe("# Starter");
    expect(store.dirty).toBe(true);
  });

  it("save sends full tokens including unknown keys", async () => {
    const { store, fns } = setup();
    await store.open("p");
    store.setTokens({ ...store.draftTokens, colors: { primary: "#fff" } });
    expect(store.dirty).toBe(true);
    await store.save();
    expect(fns.saveDesign).toHaveBeenCalledWith({
      project: "p", branch: "content/a", hash: "h1", tokens: { components: { x: 1 }, colors: { primary: "#fff" } }, body: "# Design",
    });
    expect(store.doc?.hash).toBe("h2");
    expect(store.dirty).toBe(false);
  });

  it("save on new file omits hash", async () => {
    const { store, fns } = setup({ design: vi.fn(async () => missing()) });
    await store.open("p");
    store.create();
    await store.save();
    const sent = fns.saveDesign.mock.calls[0]![0] as Record<string, unknown>;
    expect(sent).toEqual({ project: "p", branch: "main", tokens: { colors: { primary: "#111" } }, body: "# Starter" });
    expect("hash" in sent).toBe(false);
  });

  it("CHANGE_REQUIRED sets needsChange and keeps the draft; resumeSave saves with the new branch", async () => {
    const saveDesign = vi.fn()
      .mockRejectedValueOnce(new ApiError("CHANGE_REQUIRED", "create a change", 409))
      .mockResolvedValueOnce(doc({ hash: "h2", branch: "c/1" }));
    const { store } = setup({ saveDesign });
    await store.open("p");
    store.setBody("# Edited");
    await expect(store.save()).rejects.toBeInstanceOf(ApiError);
    expect(store.needsChange).toBe(true);
    expect(store.draftBody).toBe("# Edited");
    expect(store.dirty).toBe(true);
    await store.resumeSave("c/1");
    expect(saveDesign).toHaveBeenLastCalledWith(expect.objectContaining({ branch: "c/1", hash: "h1", body: "# Edited" }));
    expect(store.needsChange).toBe(false);
    expect(store.doc?.branch).toBe("c/1");
    expect(store.dirty).toBe(false);
  });

  it("stale_content sets conflict and keeps the draft", async () => {
    const saveDesign = vi.fn().mockRejectedValue(new ApiError("CONFLICT", "file changed", 409, { reason: "stale_content" }));
    const { store } = setup({ saveDesign });
    await store.open("p");
    store.setBody("# Edited");
    await expect(store.save()).rejects.toBeInstanceOf(ApiError);
    expect(store.conflict).toEqual({ reason: "stale_content", message: "file changed" });
    expect(store.draftBody).toBe("# Edited");
  });

  it("invalid_design sets invalid on open and on save", async () => {
    const invalid = new ApiError("BADREQ", "bad frontmatter", 400, { reason: "invalid_design" });
    const { store } = setup({ design: vi.fn().mockRejectedValueOnce(invalid).mockResolvedValue(doc()), saveDesign: vi.fn().mockRejectedValue(invalid) });
    await store.open("p");
    expect(store.invalid).toBe("bad frontmatter");
    expect(store.doc).toBeNull();
    await store.reload();
    expect(store.invalid).toBeNull();
    store.setBody("# Edited");
    await expect(store.save()).rejects.toBeInstanceOf(ApiError);
    expect(store.invalid).toBe("bad frontmatter");
    expect(store.draftBody).toBe("# Edited");
  });

  it("invalid_design keeps the raw source from the error details", async () => {
    const invalid = new ApiError("BADREQ", "bad frontmatter", 400, { reason: "invalid_design", details: [{ path: ["source"], message: "---\n: [\n---\n" }] });
    const { store } = setup({ design: vi.fn().mockRejectedValueOnce(invalid).mockResolvedValue(doc()) });
    await store.open("p");
    expect(store.invalid).toBe("bad frontmatter");
    expect(store.invalidSource).toBe("---\n: [\n---\n");
    await store.reload();
    expect(store.invalid).toBeNull();
    expect(store.invalidSource).toBeNull();
  });

  it("reloadFromDisk replaces a dirty draft after confirming, clearing the conflict, and the next save sends the new hash", async () => {
    const saveDesign = vi.fn()
      .mockRejectedValueOnce(new ApiError("CONFLICT", "file changed", 409, { reason: "stale_content" }))
      .mockResolvedValueOnce(doc({ body: "# Again", hash: "h10", branch: "content/b" }));
    const design = vi.fn().mockResolvedValueOnce(doc()).mockResolvedValue(doc({ body: "# Newer", hash: "h9", branch: "content/b" }));
    const { store } = setup({ design, saveDesign });
    await store.open("p");
    store.setBody("# Edited");
    await expect(store.save()).rejects.toBeInstanceOf(ApiError);
    expect(store.conflict).not.toBeNull();
    const confirm = vi.fn(() => false);
    window.confirm = confirm;
    await store.reloadFromDisk();
    expect(confirm).toHaveBeenCalledWith("Discard unsaved design changes?");
    expect(store.draftBody).toBe("# Edited");
    expect(store.conflict).not.toBeNull();
    expect(design).toHaveBeenCalledTimes(1);
    confirm.mockReturnValue(true);
    await store.reloadFromDisk();
    expect(store.draftBody).toBe("# Newer");
    expect(store.doc?.hash).toBe("h9");
    expect(store.dirty).toBe(false);
    expect(store.conflict).toBeNull();
    expect(store.changedOnDisk).toBe(false);
    store.setBody("# Again");
    await store.save();
    expect(saveDesign).toHaveBeenLastCalledWith(expect.objectContaining({ hash: "h9", branch: "content/b", body: "# Again" }));
  });

  it("reloadFromDisk does not ask when the draft is clean, and keeps the draft when the read fails", async () => {
    const design = vi.fn().mockResolvedValueOnce(doc()).mockResolvedValueOnce(doc({ body: "# Newer", hash: "h9" })).mockRejectedValueOnce(new ApiError("IO", "boom", 500));
    const { store } = setup({ design });
    const confirm = vi.fn(() => true);
    window.confirm = confirm;
    await store.open("p");
    await store.reloadFromDisk();
    expect(confirm).not.toHaveBeenCalled();
    expect(store.draftBody).toBe("# Newer");
    store.setBody("# Edited");
    await store.reloadFromDisk();
    expect(store.draftBody).toBe("# Edited");
    expect(store.error).toBe("boom");
  });

  it("refreshForBranch reloads a clean draft from the new branch", async () => {
    const design = vi.fn().mockResolvedValueOnce(doc()).mockResolvedValue(doc({ body: "# Other branch", hash: "h5", branch: "content/b" }));
    const { store } = setup({ design });
    await store.open("p");
    await store.refreshForBranch("content/b");
    expect(design).toHaveBeenCalledTimes(2);
    expect(store.doc?.branch).toBe("content/b");
    expect(store.draftBody).toBe("# Other branch");
    expect(store.changedOnDisk).toBe(false);
  });

  it("refreshForBranch keeps a dirty draft and flags changedOnDisk", async () => {
    const { store, fns } = setup();
    await store.open("p");
    store.setBody("# Edited");
    await store.refreshForBranch("content/b");
    expect(store.draftBody).toBe("# Edited");
    expect(store.changedOnDisk).toBe(true);
    expect(fns.design).toHaveBeenCalledTimes(1);
  });

  it("refreshForBranch does nothing before a design is open", async () => {
    const { store, fns } = setup();
    await store.refreshForBranch("content/b");
    expect(fns.design).not.toHaveBeenCalled();
  });

  it("a save that fails after switching projects leaves the new project's state alone", async () => {
    let reject!: (e: unknown) => void;
    const saveDesign = vi.fn(() => new Promise<DesignDoc>((_, r) => { reject = r; }));
    const design = vi.fn(async (id: string) => id === "p" ? doc() : doc({ body: "# Q", hash: "q1", branch: "q/b" }));
    const { store } = setup({ design, saveDesign });
    window.confirm = () => true;
    await store.open("p");
    store.setBody("# Edited");
    const pending = store.save();
    const settled = expect(pending).rejects.toBeInstanceOf(ApiError);
    await store.open("q");
    expect(store.saving).toBe(false);
    reject(new ApiError("CHANGE_REQUIRED", "create a change", 409));
    await settled;
    expect(store.projectId).toBe("q");
    expect(store.needsChange).toBe(false);
    expect(store.error).toBeNull();
    expect(store.conflict).toBeNull();
    expect(store.invalid).toBeNull();
    expect(store.saving).toBe(false);
    expect(store.draftBody).toBe("# Q");
  });

  it("a save finishing after switching projects does not touch the new project's doc", async () => {
    let resolve!: (d: DesignDoc) => void;
    const saveDesign = vi.fn(() => new Promise<DesignDoc>(r => { resolve = r; }));
    const design = vi.fn(async (id: string) => id === "p" ? doc() : doc({ body: "# Q", hash: "q1", branch: "q/b" }));
    const { store } = setup({ design, saveDesign });
    window.confirm = () => true;
    await store.open("p");
    store.setBody("# Edited");
    const pending = store.save();
    await store.open("q");
    resolve(doc({ body: "# Edited", hash: "h2" }));
    await pending;
    expect(store.doc?.hash).toBe("q1");
    expect(store.draftBody).toBe("# Q");
  });

  it("a second save while one is in flight shares it instead of sending another request", async () => {
    let resolve!: (d: DesignDoc) => void;
    const saveDesign = vi.fn(() => new Promise<DesignDoc>(r => { resolve = r; }));
    const { store } = setup({ saveDesign });
    await store.open("p");
    store.setBody("# Edited");
    const first = store.save();
    const second = store.save();
    expect(store.saving).toBe(true);
    resolve(doc({ body: "# Edited", hash: "h2" }));
    const [a, b] = await Promise.all([first, second]);
    expect(saveDesign).toHaveBeenCalledTimes(1);
    expect(a).toBe(b);
    expect(store.doc?.hash).toBe("h2");
    expect(store.saving).toBe(false);
  });

  it("a project opened mid-save can save without joining the old request", async () => {
    const resolvers: ((d: DesignDoc) => void)[] = [];
    const saveDesign = vi.fn(() => new Promise<DesignDoc>(r => { resolvers.push(r); }));
    const design = vi.fn(async (id: string) => id === "p" ? doc() : doc({ body: "# Q", hash: "q1", branch: "q/b" }));
    const { store } = setup({ design, saveDesign });
    window.confirm = () => true;
    await store.open("p");
    store.setBody("# Edited");
    const old = store.save();
    await store.open("q");
    store.setBody("# Q edited");
    const next = store.save();
    expect(saveDesign).toHaveBeenCalledTimes(2);
    expect(saveDesign).toHaveBeenLastCalledWith(expect.objectContaining({ project: "q", hash: "q1", body: "# Q edited" }));
    resolvers[0]!(doc({ hash: "h2" })); await old;
    expect(store.saving).toBe(true); // the old request must not clear the new one's flag
    resolvers[1]!(doc({ body: "# Q edited", hash: "q2", branch: "q/b" })); await next;
    expect(store.doc?.hash).toBe("q2");
    expect(store.saving).toBe(false);
  });

  it("checkpoint while clean reloads", async () => {
    const design = vi.fn().mockResolvedValueOnce(doc()).mockResolvedValue(doc({ body: "# Newer", hash: "h9" }));
    const { store } = setup({ design });
    await store.open("p");
    h.daemon.emit({ ...checkpoint, projectId: "other" });
    await vi.waitFor(() => expect(design).toHaveBeenCalledTimes(1));
    h.daemon.emit(checkpoint);
    await vi.waitFor(() => expect(store.draftBody).toBe("# Newer"));
    expect(store.doc?.hash).toBe("h9");
    expect(store.changedOnDisk).toBe(false);
  });

  it("checkpoint while dirty flags changedOnDisk", async () => {
    const design = vi.fn().mockResolvedValueOnce(doc()).mockResolvedValueOnce(doc()).mockResolvedValue(doc({ body: "# Newer", hash: "h9" }));
    const { store } = setup({ design });
    await store.open("p");
    store.setBody("# Edited");
    h.daemon.emit(checkpoint);
    await vi.waitFor(() => expect(design).toHaveBeenCalledTimes(2));
    await Promise.resolve();
    expect(store.changedOnDisk).toBe(false); // same hash: nothing changed under the draft
    h.daemon.emit(checkpoint);
    await vi.waitFor(() => expect(store.changedOnDisk).toBe(true));
    expect(store.draftBody).toBe("# Edited");
    expect(store.doc?.hash).toBe("h1");
  });

  it("canLeave confirms before discarding a dirty draft", async () => {
    const { store } = setup();
    await store.open("p");
    expect(store.canLeave()).toBe(true);
    store.setBody("# Edited");
    const confirm = vi.fn();
    window.confirm = confirm;
    confirm.mockReturnValueOnce(false);
    expect(store.canLeave()).toBe(false);
    expect(store.draftBody).toBe("# Edited");
    confirm.mockReturnValueOnce(true);
    expect(store.canLeave()).toBe(true);
    expect(store.draftBody).toBe("# Design");
  });
});
