// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import type { SkillDoc, SkillMeta, SkillsList } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
// The real editor loads Tiptap and CodeMirror; the stub shows its body and emits one edit.
vi.mock("@/modules/content/MarkdownBodyEditor.vue", async () => {
  const { defineComponent, h: el } = await import("vue");
  return {
    __esModule: true,
    default: defineComponent({
      props: { body: String, original: String, kind: String, rich: Boolean },
      emits: ["change"],
      setup: (props, { emit }) => () => el("div", { "data-testid": "body-editor" }, [
        el("span", { "data-testid": "body-text" }, props.body),
        el("button", { "data-testid": "body-edit", onClick: () => emit("change", { body: "# Edited", dirty: true }) }),
      ]),
    }),
  };
});

// Creating the change is NewChangeDialog's job; the stub only reports one created.
vi.mock("@/modules/changes/NewChangeDialog.vue", async () => {
  const { defineComponent, h: el } = await import("vue");
  return {
    __esModule: true,
    default: defineComponent({
      props: { open: Boolean, projectId: String },
      emits: ["created", "update:open"],
      setup: (props, { emit }) => () => props.open
        ? el("button", { "data-testid": "new-change-stub", onClick: () => emit("created", { branch: "content/new" }) })
        : null,
    }),
  };
});

import SkillsPanel from "./SkillsPanel.vue";
import { useSkillsStore } from "./store";

const meta = (over: Partial<SkillMeta> = {}): SkillMeta => ({
  id: "seo", name: "SEO", description: "SEO guidance", scope: "project", path: ".orso/skills/seo/SKILL.md", ...over,
});
const doc = (over: Partial<SkillDoc> = {}): SkillDoc => ({
  id: "seo", name: "SEO", description: "SEO guidance", body: "# SEO", hash: "h1", scope: "project", path: ".orso/skills/seo/SKILL.md", valid: true, error: null, ...over,
});
const listPayload = (installed: SkillMeta[] = [meta()]): SkillsList => ({ installed, catalog: [] });

async function setup(api: Record<string, unknown> = {}) {
  h.daemon = createFakeDaemon({
    skills: vi.fn(async () => listPayload()),
    skill: vi.fn(async () => doc()),
    activeChange: vi.fn(async () => ({ branch: "content/a", title: "Edit skills" })),
    saveSkill: vi.fn(async () => doc({ hash: "h2" })),
    ...api,
  });
  setActivePinia(createPinia());
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/p/:id/skills", component: SkillsPanel, props: r => ({ projectId: r.params.id as string }) },
      { path: "/other", component: { template: "<p>Other</p>" } },
    ],
  });
  await router.push("/p/site/skills");
  await router.isReady();
  const w = mount(RouterView, { global: { plugins: [router] } });
  await flushPromises();
  const fns = h.daemon.api as unknown as Record<string, ReturnType<typeof vi.fn>>;
  return { w, router, fns };
}
const button = (w: Awaited<ReturnType<typeof setup>>["w"], id: string) => w.get(`[data-testid="${id}"]`);

beforeEach(() => { vi.clearAllMocks(); });

describe("SkillsPanel", () => {
  it("with no installed skills shows the install CTA", async () => {
    const { w } = await setup({ skills: vi.fn(async () => ({ installed: [], catalog: [] })) });
    expect(w.get('[data-testid="skills-empty"]').text()).toContain("Install");
    expect(w.find('[data-testid="skills-editor"]').exists()).toBe(false);
    await button(w, "skills-install").trigger("click");
    expect(w.find('[data-testid="skills-install-stub"]').exists()).toBe(true);
    w.unmount();
  });

  it("selecting a sidebar row loads and shows its body", async () => {
    const { w } = await setup();
    await button(w, "skills-sidebar-row").trigger("click");
    await flushPromises();
    expect(w.get('[data-testid="skills-editor"]').exists()).toBe(true);
    expect(w.get('[data-testid="body-text"]').text()).toBe("# SEO");
    expect(button(w, "skills-name").element.value).toBe("SEO");
    expect(button(w, "skills-description").element.value).toBe("SEO guidance");
    expect(w.get('[data-testid="skills-path"]').text()).toContain(".orso/skills/seo/SKILL.md");
    w.unmount();
  });

  it("a project save refused on main disables Save and shows Create change", async () => {
    const { ApiError } = await import("@/core/api");
    const saveSkill = vi.fn()
      .mockRejectedValueOnce(new ApiError("CHANGE_REQUIRED", "Start a change first", 409))
      .mockResolvedValueOnce(doc({ body: "# Edited", hash: "h2" }));
    const { w } = await setup({ saveSkill });
    await button(w, "skills-sidebar-row").trigger("click");
    await flushPromises();
    await button(w, "body-edit").trigger("click");
    await button(w, "skills-save").trigger("click");
    await flushPromises();
    expect(w.get('[data-testid="skills-needs-change"]').text()).toContain("Create change");
    expect(button(w, "skills-save").attributes("disabled")).toBeDefined();
    await button(w, "skills-create-change").trigger("click");
    await button(w, "new-change-stub").trigger("click");
    await flushPromises();
    expect(saveSkill).toHaveBeenLastCalledWith("seo", expect.objectContaining({ branch: "content/new", body: "# Edited" }));
    expect(w.find('[data-testid="skills-needs-change"]').exists()).toBe(false);
    expect(useSkillsStore().dirty).toBe(false);
    w.unmount();
  });

  it("a global skill's Save is enabled when dirty", async () => {
    const skills = vi.fn(async () => ({ installed: [meta({ scope: "global" })], catalog: [] }));
    const skill = vi.fn(async () => doc({ scope: "global" }));
    const { w } = await setup({ skills, skill });
    await button(w, "skills-sidebar-row").trigger("click");
    await flushPromises();
    expect(button(w, "skills-save").attributes("disabled")).toBeDefined();
    await button(w, "body-edit").trigger("click");
    expect(button(w, "skills-save").attributes("disabled")).toBeUndefined();
    w.unmount();
  });
});
