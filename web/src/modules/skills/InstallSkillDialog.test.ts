// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DOMWrapper, flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import type { CatalogItem, SkillDoc, SkillMeta, SkillScope, SkillsList } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import InstallSkillDialog from "./InstallSkillDialog.vue";
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

let wrapper: VueWrapper | null = null;
const $ = <T extends HTMLElement = HTMLElement>(sel: string) =>
  document.body.querySelector(sel) as T | null;

async function setup(api: Record<string, unknown> = {}) {
  h.daemon = createFakeDaemon({
    skills: vi.fn(async () => listPayload()),
    skill: vi.fn(async (_p: string, _id: string, scope: SkillScope) => doc({ id: "seo", scope })),
    activeChange: vi.fn(async () => ({ branch: "content/a", title: "Edit skills" })),
    installSkill: vi.fn(async (body: { scope: SkillScope }) => meta({ id: "seo", scope: body.scope })),
    ...api,
  });
  setActivePinia(createPinia());
  const store = useSkillsStore();
  await store.open("site");
  const fns = h.daemon.api as unknown as Record<string, ReturnType<typeof vi.fn>>;
  return { store, fns };
}

function mountDialog() {
  wrapper = mount(InstallSkillDialog, { props: { open: true }, attachTo: document.body });
  return wrapper;
}

const installButton = (id: string) =>
  $<HTMLButtonElement>(`[data-testid="install-catalog-install"][data-skill-id="${id}"]`)!;

/** reka-ui tabs activate on mousedown, not click. */
async function openImportTab() {
  await new DOMWrapper($('[data-testid="install-segment-import"]')!).trigger("mousedown", { button: 0 });
  await flushPromises();
}

beforeEach(() => {
  vi.restoreAllMocks();
  document.body.innerHTML = "";
});

afterEach(() => {
  wrapper?.unmount();
  wrapper = null;
  document.body.innerHTML = "";
});

describe("InstallSkillDialog", () => {
  it("installs a catalog skill into the project scope by default", async () => {
    const { fns } = await setup();
    const w = mountDialog();
    await flushPromises();

    installButton("seo").click();
    await flushPromises();

    expect(fns.installSkill).toHaveBeenCalledWith({
      project: "site", branch: "content/a", scope: "project", source: { catalog: { id: "seo" } },
    });
    expect(w.emitted("update:open")?.at(-1)).toEqual([false]);
  });

  it("disables a catalog Install when the skill is already installed in the chosen scope", async () => {
    await setup({
      skills: vi.fn(async () => ({
        installed: [meta()],
        catalog: [catalogItem({ installedProject: true }), catalogItem({ id: "design", name: "Design" })],
      })),
    });
    mountDialog();
    await flushPromises();

    expect(installButton("seo").disabled).toBe(true);
    expect(installButton("design").disabled).toBe(false);
  });

  it("installs into the global scope when Global is chosen", async () => {
    const { fns } = await setup();
    const w = mountDialog();
    await flushPromises();

    $('[data-testid="install-scope-global"]')!.click();
    await flushPromises();
    installButton("seo").click();
    await flushPromises();

    expect(fns.installSkill).toHaveBeenCalledWith({
      scope: "global", source: { catalog: { id: "seo" } },
    });
    expect(w.emitted("update:open")?.at(-1)).toEqual([false]);
  });

  it("disables a catalog Install already installed globally when Global is chosen", async () => {
    await setup({
      skills: vi.fn(async () => ({
        installed: [meta({ scope: "global" })],
        catalog: [catalogItem({ installedGlobal: true })],
      })),
    });
    mountDialog();
    await flushPromises();

    expect(installButton("seo").disabled).toBe(false);
    $('[data-testid="install-scope-global"]')!.click();
    await flushPromises();
    expect(installButton("seo").disabled).toBe(true);
  });

  it("closes when a project install is refused on main", async () => {
    const { ApiError } = await import("@/core/api");
    const installSkill = vi.fn().mockRejectedValue(new ApiError("CHANGE_REQUIRED", "Start a change first", 409));
    const { store } = await setup({ installSkill });
    const w = mountDialog();
    await flushPromises();

    installButton("seo").click();
    await flushPromises();

    expect(store.needsChange).toBe(true);
    expect(w.emitted("update:open")?.at(-1)).toEqual([false]);
    expect($('[data-testid="install-skill-dialog"]')).toBeNull();
  });

  it("shows a plain install error in the dialog and stays open", async () => {
    const { ApiError } = await import("@/core/api");
    const installSkill = vi.fn().mockRejectedValue(
      new ApiError("CONFLICT", "skill seo is already installed", 409, { reason: "duplicate_slug" }),
    );
    await setup({ installSkill });
    mountDialog();
    await flushPromises();

    installButton("seo").click();
    await flushPromises();

    expect($('[data-testid="install-error"]')!.textContent).toContain("already installed");
    expect($('[data-testid="install-skill-dialog"]')).not.toBeNull();
  });

  it("shows a conflict failure in the dialog", async () => {
    const { ApiError } = await import("@/core/api");
    const installSkill = vi.fn().mockRejectedValue(
      new ApiError("CONFLICT", "branch changed", 409, { reason: "branch_changed" }),
    );
    const { store } = await setup({ installSkill });
    mountDialog();
    await flushPromises();

    installButton("seo").click();
    await flushPromises();

    expect(store.conflict?.message).toBe("branch changed");
    expect($('[data-testid="install-error"]')!.textContent).toContain("branch changed");
    expect($('[data-testid="install-skill-dialog"]')).not.toBeNull();
  });

  it("reads a chosen .md file and installs it as an import", async () => {
    const { fns } = await setup();
    const w = mountDialog();
    await flushPromises();

    await openImportTab();

    const text = "---\nname: Imported\ndescription: Imported skill\n---\nBody here.\n";
    const file = new File([text], "custom.md", { type: "text/markdown" });
    const input = $<HTMLInputElement>('[data-testid="install-import-file"]')!;
    Object.defineProperty(input, "files", { value: [file], configurable: true });
    input.dispatchEvent(new Event("change"));
    await vi.waitFor(() => {
      expect($<HTMLButtonElement>('[data-testid="install-import-install"]')!.disabled).toBe(false);
    });

    $('[data-testid="install-import-install"]')!.click();
    await flushPromises();

    expect(fns.installSkill).toHaveBeenCalledWith({
      project: "site", branch: "content/a", scope: "project", source: { import: { skillMd: text } },
    });
    expect(w.emitted("update:open")?.at(-1)).toEqual([false]);
  });

  it("shows a hint when the catalog is empty", async () => {
    await setup({ skills: vi.fn(async () => ({ installed: [], catalog: [] })) });
    mountDialog();
    await flushPromises();

    expect($('[data-testid="install-catalog-empty"]')!.textContent).toContain("No catalog skills available");
  });

  it("filters the catalog with the search field", async () => {
    await setup();
    mountDialog();
    await flushPromises();

    const search = $<HTMLInputElement>('[data-testid="install-search"]')!;
    search.value = "Design";
    search.dispatchEvent(new Event("input"));
    await flushPromises();

    expect(installButton("seo")).toBeNull();
    expect(installButton("design")).not.toBeNull();
  });

  it("New skill opens the blank form", async () => {
    await setup();
    mountDialog();
    await flushPromises();

    $<HTMLButtonElement>('[data-testid="install-new"]')!.click();
    await flushPromises();

    expect($('[data-testid="install-blank-name"]')).not.toBeNull();
    expect($('[data-testid="install-blank-description"]')).not.toBeNull();
  });

  it("creates a new skill from the New skill editor", async () => {
    const { fns } = await setup();
    mountDialog();
    await flushPromises();

    $<HTMLButtonElement>('[data-testid="install-new"]')!.click();
    await flushPromises();

    const name = $<HTMLInputElement>('[data-testid="install-blank-name"]')!;
    name.value = "Brand voice";
    name.dispatchEvent(new Event("input"));
    const description = $<HTMLTextAreaElement>('[data-testid="install-blank-description"]')!;
    description.value = "How we sound";
    description.dispatchEvent(new Event("input"));
    await flushPromises();

    const create = $<HTMLButtonElement>('[data-testid="install-blank-create"]')!;
    expect(create.disabled).toBe(false);
    create.click();
    await flushPromises();

    expect(fns.installSkill).toHaveBeenCalledWith({
      project: "site", branch: "content/a", scope: "project",
      source: { blank: { name: "Brand voice", description: "How we sound" } },
    });
  });

  it("clears a stale error when the dialog reopens", async () => {
    const { store } = await setup();
    const w = mount(InstallSkillDialog, { props: { open: false }, attachTo: document.body });
    wrapper = w;
    store.error = "old failure";

    await w.setProps({ open: true });
    await flushPromises();

    expect($('[data-testid="install-error"]')).toBeNull();
    expect(store.error).toBeNull();
  });
});
