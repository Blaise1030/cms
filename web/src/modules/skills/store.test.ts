// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import { ApiError } from "@/core/api";
import type { CatalogItem, SkillDoc, SkillMeta, SkillsList } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
import { useSkillsStore } from "./store";

const meta = (over: Partial<SkillMeta> = {}): SkillMeta => ({
  id: "seo", name: "SEO", description: "SEO guidance", scope: "project", path: ".orso/skills/seo/SKILL.md", ...over,
});
const doc = (over: Partial<SkillDoc> = {}): SkillDoc => ({
  id: "seo", name: "SEO", description: "SEO guidance", body: "# SEO", hash: "h1", scope: "project", path: ".orso/skills/seo/SKILL.md", valid: true, error: null, ...over,
});
const catalogItem = (over: Partial<CatalogItem> = {}): CatalogItem => ({
  id: "seo", name: "SEO", description: "SEO guidance", installedProject: false, installedGlobal: false, ...over,
});
const listPayload = (): SkillsList => ({
  installed: [meta()],
  catalog: [catalogItem(), catalogItem({ id: "design", name: "Design" })],
});

function setup(api: Record<string, unknown> = {}) {
  h.daemon = createFakeDaemon({
    skills: vi.fn(async () => listPayload()),
    skill: vi.fn(async () => doc()),
    activeChange: vi.fn(async () => ({ branch: "content/a", title: "Edit skills" })),
    saveSkill: vi.fn(async () => doc({ hash: "h2" })),
    ...api,
  });
  setActivePinia(createPinia());
  const store = useSkillsStore();
  const fns = h.daemon.api as unknown as Record<string, ReturnType<typeof vi.fn>>;
  return { store, fns };
}

beforeEach(() => { vi.restoreAllMocks(); });

describe("skills store", () => {
  it("open lists installed skills and the catalog", async () => {
    const { store, fns } = setup();
    await store.open("p");
    expect(fns.skills).toHaveBeenCalledWith("p");
    expect(store.list).toEqual([meta()]);
    expect(store.catalog).toHaveLength(2);
    expect(store.selected).toBeNull();
  });

  it("select loads the doc and drafts", async () => {
    const { store, fns } = setup();
    await store.open("p");
    await store.select("seo", "project");
    expect(fns.skill).toHaveBeenCalledWith("p", "seo", "project");
    expect(store.selected).toEqual({ id: "seo", scope: "project" });
    expect(store.doc?.hash).toBe("h1");
    expect(store.draftName).toBe("SEO");
    expect(store.draftDescription).toBe("SEO guidance");
    expect(store.draftBody).toBe("# SEO");
    expect(store.dirty).toBe(false);
    store.setBody("# Edited");
    expect(store.dirty).toBe(true);
    store.setBody("# SEO");
    expect(store.dirty).toBe(false);
  });

  it("project save sends the project and the active branch", async () => {
    const saveSkill = vi.fn(async () => doc({ hash: "h2" }));
    const { store } = setup({ saveSkill });
    await store.open("p");
    await store.select("seo", "project");
    store.setBody("# Edited");
    await store.save();
    expect(saveSkill).toHaveBeenLastCalledWith("seo", {
      project: "p", branch: "content/a", scope: "project", name: "SEO", description: "SEO guidance", body: "# Edited", hash: "h1",
    });
    expect(store.doc?.hash).toBe("h2");
    expect(store.dirty).toBe(false);
  });

  it("global save omits project and branch", async () => {
    const saveSkill = vi.fn(async () => doc({ scope: "global", hash: "h2" }));
    const skill = vi.fn(async () => doc({ scope: "global" }));
    const { store } = setup({ saveSkill, skill });
    await store.open("p");
    await store.select("seo", "global");
    store.setBody("# Edited");
    await store.save();
    expect(saveSkill).toHaveBeenLastCalledWith("seo", {
      scope: "global", name: "SEO", description: "SEO guidance", body: "# Edited", hash: "h1",
    });
  });

  it("project save maps CHANGE_REQUIRED to needsChange and keeps the draft", async () => {
    const saveSkill = vi.fn().mockRejectedValue(new ApiError("CHANGE_REQUIRED", "create a change", 409));
    const { store } = setup({ saveSkill });
    await store.open("p");
    await store.select("seo", "project");
    store.setBody("# Edited");
    await expect(store.save()).rejects.toBeInstanceOf(ApiError);
    expect(store.needsChange).toBe(true);
    expect(store.draftBody).toBe("# Edited");
    expect(store.dirty).toBe(true);
  });

  it("global save never sets needsChange", async () => {
    const saveSkill = vi.fn().mockRejectedValue(new ApiError("CHANGE_REQUIRED", "create a change", 409));
    const skill = vi.fn(async () => doc({ scope: "global" }));
    const { store } = setup({ saveSkill, skill });
    await store.open("p");
    await store.select("seo", "global");
    store.setBody("# Edited");
    await expect(store.save()).rejects.toBeInstanceOf(ApiError);
    expect(store.needsChange).toBe(false);
    expect(store.draftBody).toBe("# Edited");
  });

  it("resumeSave retries the save on the new branch", async () => {
    const saveSkill = vi.fn()
      .mockRejectedValueOnce(new ApiError("CHANGE_REQUIRED", "create a change", 409))
      .mockResolvedValueOnce(doc({ hash: "h2" }));
    const { store } = setup({ saveSkill });
    await store.open("p");
    await store.select("seo", "project");
    store.setBody("# Edited");
    await expect(store.save()).rejects.toBeInstanceOf(ApiError);
    expect(store.needsChange).toBe(true);
    await store.resumeSave("c/1");
    expect(saveSkill).toHaveBeenLastCalledWith("seo", expect.objectContaining({ branch: "c/1", body: "# Edited" }));
    expect(store.needsChange).toBe(false);
    expect(store.doc?.hash).toBe("h2");
    expect(store.dirty).toBe(false);
  });

  it("stale_content sets conflict and keeps the draft", async () => {
    const saveSkill = vi.fn().mockRejectedValue(new ApiError("CONFLICT", "file changed", 409, { reason: "stale_content" }));
    const { store } = setup({ saveSkill });
    await store.open("p");
    await store.select("seo", "project");
    store.setBody("# Edited");
    await expect(store.save()).rejects.toBeInstanceOf(ApiError);
    expect(store.conflict).toEqual({ reason: "stale_content", message: "file changed" });
    expect(store.draftBody).toBe("# Edited");
  });

  it("invalid_skill sets invalid on select and keeps the doc empty", async () => {
    const invalid = new ApiError("BADREQ", "bad frontmatter", 400, { reason: "invalid_skill" });
    const { store } = setup({ skill: vi.fn().mockRejectedValue(invalid) });
    await store.open("p");
    await store.select("seo", "project");
    expect(store.invalid).toBe("bad frontmatter");
    expect(store.doc).toBeNull();
  });

  it("install calls the API then refreshes the list and selects the installed skill", async () => {
    const installSkill = vi.fn(async () => meta({ id: "design", name: "Design", scope: "global" }));
    const skill = vi.fn(async () => doc({ id: "design", scope: "global" }));
    const { store } = setup({ installSkill, skill });
    await store.open("p");
    await store.install("global", { catalog: { id: "design" } });
    expect(installSkill).toHaveBeenCalledWith({ scope: "global", source: { catalog: { id: "design" } } });
    expect(store.selected).toEqual({ id: "design", scope: "global" });
    expect(store.doc?.id).toBe("design");
  });

  it("install maps a project CHANGE_REQUIRED to needsChange", async () => {
    const installSkill = vi.fn().mockRejectedValue(new ApiError("CHANGE_REQUIRED", "create a change", 409));
    const { store } = setup({ installSkill });
    await store.open("p");
    await expect(store.install("project", { catalog: { id: "seo" } })).rejects.toBeInstanceOf(ApiError);
    expect(store.needsChange).toBe(true);
    expect(store.error).toBeNull();
  });

  it("uninstall maps a project CHANGE_REQUIRED to needsChange", async () => {
    const uninstallSkill = vi.fn().mockRejectedValue(new ApiError("CHANGE_REQUIRED", "create a change", 409));
    const { store } = setup({ uninstallSkill });
    await store.open("p");
    await store.select("seo", "project");
    await expect(store.uninstall()).rejects.toBeInstanceOf(ApiError);
    expect(store.needsChange).toBe(true);
    expect(store.error).toBeNull();
    expect(store.selected).toEqual({ id: "seo", scope: "project" });
  });

  it("uninstall maps stale_content to conflict", async () => {
    const uninstallSkill = vi.fn().mockRejectedValue(new ApiError("CONFLICT", "file changed", 409, { reason: "stale_content" }));
    const { store } = setup({ uninstallSkill });
    await store.open("p");
    await store.select("seo", "project");
    await expect(store.uninstall()).rejects.toBeInstanceOf(ApiError);
    expect(store.conflict).toEqual({ reason: "stale_content", message: "file changed" });
    expect(store.error).toBeNull();
  });

  it("reload keeps a dirty draft when the selected skill disappears", async () => {
    const skills = vi.fn().mockResolvedValueOnce(listPayload()).mockResolvedValue({ installed: [], catalog: [] });
    const { store } = setup({ skills });
    await store.open("p");
    await store.select("seo", "project");
    store.setBody("# Edited");
    await store.reload();
    expect(store.selected).toEqual({ id: "seo", scope: "project" });
    expect(store.doc?.id).toBe("seo");
    expect(store.draftBody).toBe("# Edited");
    expect(store.dirty).toBe(true);
    expect(store.changedOnDisk).toBe(true);
  });

  it("reload clears a clean selection when the skill disappears", async () => {
    const skills = vi.fn().mockResolvedValueOnce(listPayload()).mockResolvedValue({ installed: [], catalog: [] });
    const { store } = setup({ skills });
    await store.open("p");
    await store.select("seo", "project");
    await store.reload();
    expect(store.selected).toBeNull();
    expect(store.doc).toBeNull();
  });

  it("reloadFromDisk replaces a dirty draft after confirming", async () => {
    const skill = vi.fn().mockResolvedValueOnce(doc()).mockResolvedValue(doc({ body: "# On disk", hash: "h9" }));
    const { store } = setup({ skill });
    await store.open("p");
    await store.select("seo", "project");
    store.setBody("# Edited");
    vi.stubGlobal("confirm", vi.fn(() => false));
    await store.reloadFromDisk();
    expect(store.draftBody).toBe("# Edited");
    vi.stubGlobal("confirm", vi.fn(() => true));
    await store.reloadFromDisk();
    expect(store.draftBody).toBe("# On disk");
    expect(store.doc?.hash).toBe("h9");
    vi.unstubAllGlobals();
  });

  it("resumeSave retries a refused uninstall on the new branch", async () => {
    const uninstallSkill = vi.fn()
      .mockRejectedValueOnce(new ApiError("CHANGE_REQUIRED", "create a change", 409))
      .mockResolvedValueOnce(undefined);
    const { store } = setup({ uninstallSkill });
    await store.open("p");
    await store.select("seo", "project");
    await expect(store.uninstall()).rejects.toBeInstanceOf(ApiError);
    expect(store.needsChange).toBe(true);
    await store.resumeSave("c/1");
    expect(uninstallSkill).toHaveBeenLastCalledWith("seo", { project: "p", scope: "project", branch: "c/1" });
    expect(store.needsChange).toBe(false);
    expect(store.selected).toBeNull();
    expect(store.doc).toBeNull();
  });
});
