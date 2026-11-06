// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
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
});
