// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import { ApiError } from "@/core/api";
import type { SettingsDoc } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
import { useSettingsStore } from "./store";

const defaults = { root: ".", installDir: ".", install: "npm install --no-audit --no-fund --no-progress", dev: "npm run dev -- --port {port} --host {host} --ignore-lock" };
const doc = (over: Partial<SettingsDoc> = {}): SettingsDoc => ({
  exists: true, settings: { root: "apps/web", extra: { keep: true } }, defaults, hash: "h1", branch: "content/a", errors: {}, ...over,
});

function setup(api: Record<string, unknown> = {}) {
  h.daemon = createFakeDaemon({ settings: vi.fn(async () => doc()), saveSettings: vi.fn(async () => doc({ hash: "h2" })), ...api });
  setActivePinia(createPinia());
  const store = useSettingsStore();
  const fns = h.daemon.api as unknown as Record<"settings" | "saveSettings", ReturnType<typeof vi.fn>>;
  return { store, fns };
}
const fsChange = (path: string) => ({ type: "fs.change", projectId: "p", path, kind: "modify" }) as const;
const checkpoint = { type: "checkpoint.created", projectId: "p", sha: "s", summary: "x", author: "a", ts: 1 } as const;

beforeEach(() => { vi.restoreAllMocks(); });

describe("settings store", () => {
  it("open loads the doc and drafts the four fields", async () => {
    const { store, fns } = setup();
    await store.open("p");
    expect(fns.settings).toHaveBeenCalledWith("p");
    expect(store.doc?.hash).toBe("h1");
    expect(store.draft).toEqual({ root: "apps/web", installDir: "", install: "", dev: "" });
    expect(store.dirty).toBe(false);
  });

  it("setField makes the draft dirty until it matches the file again", async () => {
    const { store } = setup();
    await store.open("p");
    store.setField("dev", "pnpm dev --port {port}");
    expect(store.dirty).toBe(true);
    store.setField("dev", "");
    expect(store.dirty).toBe(false);
  });

  it("save sends only set fields plus unknown keys and the hash", async () => {
    const { store, fns } = setup();
    await store.open("p");
    store.setField("root", "");
    store.setField("dev", "pnpm dev --port {port}");
    await store.save();
    expect(fns.saveSettings).toHaveBeenCalledWith({
      project: "p", branch: "content/a", hash: "h1", settings: { extra: { keep: true }, dev: "pnpm dev --port {port}" },
    });
    expect(store.doc?.hash).toBe("h2");
    expect(store.dirty).toBe(false);
  });

  it("save on a missing file omits the hash", async () => {
    const { store, fns } = setup({ settings: vi.fn(async () => doc({ exists: false, settings: {}, hash: undefined })) });
    await store.open("p");
    store.setField("install", "pnpm install");
    await store.save();
    const sent = fns.saveSettings.mock.calls[0]![0] as Record<string, unknown>;
    expect(sent).toEqual({ project: "p", branch: "content/a", settings: { install: "pnpm install" } });
    expect("hash" in sent).toBe(false);
  });

  it("CHANGE_REQUIRED sets needsChange and keeps the draft; resumeSave retries on the new branch", async () => {
    const saveSettings = vi.fn()
      .mockRejectedValueOnce(new ApiError("CHANGE_REQUIRED", "create a change", 409))
      .mockResolvedValueOnce(doc({ settings: { root: "apps/web", extra: { keep: true }, dev: "x {port}" }, hash: "h2", branch: "change/x" }));
    const { store } = setup({ saveSettings });
    await store.open("p");
    store.setField("dev", "x {port}");
    await expect(store.save()).rejects.toBeInstanceOf(ApiError);
    expect(store.needsChange).toBe(true);
    expect(store.draft.dev).toBe("x {port}");
    await store.resumeSave("change/x");
    expect(saveSettings).toHaveBeenLastCalledWith(expect.objectContaining({ branch: "change/x", hash: "h1" }));
    expect(store.needsChange).toBe(false);
    expect(store.doc?.branch).toBe("change/x");
    expect(store.dirty).toBe(false);
  });

  it("stale_content sets conflict and keeps the draft", async () => {
    const saveSettings = vi.fn().mockRejectedValue(new ApiError("CONFLICT", "file changed", 409, { reason: "stale_content" }));
    const { store } = setup({ saveSettings });
    await store.open("p");
    store.setField("dev", "x {port}");
    await expect(store.save()).rejects.toBeInstanceOf(ApiError);
    expect(store.conflict).toEqual({ reason: "stale_content", message: "file changed" });
    expect(store.draft.dev).toBe("x {port}");
  });

  it("fieldErrors come from the doc and are overridden by invalid_settings details", async () => {
    const saveSettings = vi.fn().mockRejectedValue(new ApiError("BADREQ", "invalid settings", 400, {
      reason: "invalid_settings", details: [{ path: ["dev"], message: "must contain {port}" }],
    }));
    const { store } = setup({ saveSettings, settings: vi.fn(async () => doc({ errors: { root: "must be a relative path" } })) });
    await store.open("p");
    expect(store.fieldErrors).toEqual({ root: "must be a relative path" });
    store.setField("dev", "pnpm dev");
    await expect(store.save()).rejects.toBeInstanceOf(ApiError);
    expect(store.fieldErrors).toEqual({ dev: "must contain {port}" });
    expect(store.error).toBeNull();
    store.discard();
    expect(store.fieldErrors).toEqual({ root: "must be a relative path" });
  });

  it("an fs.change for orso.json reloads a clean draft and ignores other paths", async () => {
    const settings = vi.fn().mockResolvedValueOnce(doc()).mockResolvedValue(doc({ settings: { root: "site" }, hash: "h9" }));
    const { store } = setup({ settings });
    await store.open("p");
    h.daemon.emit(fsChange("src/pages/index.astro"));
    h.daemon.emit({ ...fsChange("orso.json"), projectId: "other" });
    await Promise.resolve();
    expect(settings).toHaveBeenCalledTimes(1);
    h.daemon.emit(fsChange("orso.json"));
    await vi.waitFor(() => expect(store.draft.root).toBe("site"));
    expect(store.changedOnDisk).toBe(false);
  });

  it("an fs.change for orso.json with a dirty draft and a new hash flags changedOnDisk", async () => {
    const settings = vi.fn().mockResolvedValueOnce(doc()).mockResolvedValue(doc({ settings: { root: "site" }, hash: "h9" }));
    const { store } = setup({ settings });
    await store.open("p");
    store.setField("dev", "x {port}");
    h.daemon.emit(fsChange("orso.json"));
    await vi.waitFor(() => expect(store.changedOnDisk).toBe(true));
    expect(store.draft.dev).toBe("x {port}");
    expect(store.doc?.hash).toBe("h1");
  });

  it("a checkpoint with a dirty draft and the same hash leaves it alone; with a new hash flags it", async () => {
    const settings = vi.fn().mockResolvedValueOnce(doc()).mockResolvedValueOnce(doc()).mockResolvedValue(doc({ hash: "h9" }));
    const { store } = setup({ settings });
    await store.open("p");
    store.setField("dev", "x {port}");
    h.daemon.emit(checkpoint);
    await vi.waitFor(() => expect(settings).toHaveBeenCalledTimes(2));
    await Promise.resolve();
    expect(store.changedOnDisk).toBe(false);
    h.daemon.emit(checkpoint);
    await vi.waitFor(() => expect(store.changedOnDisk).toBe(true));
  });

  it("reloadFromDisk replaces a dirty draft after confirming", async () => {
    const settings = vi.fn().mockResolvedValueOnce(doc()).mockResolvedValue(doc({ settings: { root: "site" }, hash: "h9" }));
    const { store } = setup({ settings });
    await store.open("p");
    store.setField("dev", "x {port}");
    window.confirm = vi.fn(() => false);
    await store.reloadFromDisk();
    expect(window.confirm).toHaveBeenCalledWith("Discard unsaved settings changes?");
    expect(store.draft.dev).toBe("x {port}");
    window.confirm = vi.fn(() => true);
    await store.reloadFromDisk();
    expect(store.draft).toEqual({ root: "site", installDir: "", install: "", dev: "" });
    expect(store.doc?.hash).toBe("h9");
  });

  it("refreshForBranch reloads a clean draft and flags a dirty one", async () => {
    const settings = vi.fn().mockResolvedValueOnce(doc()).mockResolvedValue(doc({ branch: "content/b", hash: "h5" }));
    const { store } = setup({ settings });
    await store.open("p");
    await store.refreshForBranch("content/b");
    expect(store.doc?.branch).toBe("content/b");
    store.setField("dev", "x {port}");
    await store.refreshForBranch("content/c");
    expect(store.changedOnDisk).toBe(true);
    expect(settings).toHaveBeenCalledTimes(2);
  });

  it("canLeave confirms before discarding a dirty draft", async () => {
    const { store } = setup();
    await store.open("p");
    expect(store.canLeave()).toBe(true);
    store.setField("dev", "x {port}");
    window.confirm = vi.fn(() => false);
    expect(store.canLeave()).toBe(false);
    window.confirm = vi.fn(() => true);
    expect(store.canLeave()).toBe(true);
    expect(store.draft.dev).toBe("");
  });
});
