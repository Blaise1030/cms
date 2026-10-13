// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { toast } from "vue-sonner";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import type { DesignDoc } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("vue-sonner", () => ({ toast: { info: vi.fn(), success: vi.fn(), warning: vi.fn(), error: vi.fn() } }));
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

import DesignPanel from "./DesignPanel.vue";
import { useDesignStore } from "./store";
import { useAgentOutboxStore } from "@/modules/terminal/outbox";

const APPLY_PROMPT = "Update the @theme block in src/styles/global.css (fonts, colours, radius, spacing) to match DESIGN.md, and follow its guidance in the components.";
const doc = (over: Partial<DesignDoc> = {}): DesignDoc => ({
  exists: true, tokens: { colors: { primary: "#000" } }, body: "# Design", hash: "h1", branch: "content/a", ...over,
});
const missing = (): DesignDoc => ({
  exists: false, tokens: {}, body: "", branch: "main", starter: { tokens: { colors: { primary: "#111" } }, body: "# Starter" },
});

async function setup(api: Record<string, unknown> = {}) {
  h.daemon = createFakeDaemon({ design: vi.fn(async () => doc()), saveDesign: vi.fn(async () => doc({ hash: "h2" })), ...api });
  setActivePinia(createPinia());
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/p/:id/design", component: DesignPanel, props: r => ({ projectId: r.params.id as string }) },
      { path: "/other", component: { template: "<p>Other</p>" } },
    ],
  });
  await router.push("/p/site/design");
  await router.isReady();
  const w = mount(RouterView, { global: { plugins: [router] } });
  await flushPromises();
  const fns = h.daemon.api as unknown as Record<"design" | "saveDesign", ReturnType<typeof vi.fn>>;
  return { w, router, fns };
}
const button = (w: Awaited<ReturnType<typeof setup>>["w"], id: string) => w.get(`[data-testid="${id}"]`);

beforeEach(() => { vi.clearAllMocks(); });

describe("DesignPanel", () => {
  it("missing file shows Create, which opens the editor with the starter", async () => {
    const { w } = await setup({ design: vi.fn(async () => missing()) });
    expect(w.find('[data-testid="body-editor"]').exists()).toBe(false);
    await button(w, "design-create").trigger("click");
    await flushPromises();
    expect(w.find('[data-testid="design-create"]').exists()).toBe(false);
    expect(w.get('[data-testid="body-text"]').text()).toBe("# Starter");
    expect(w.find('[data-testid="design-brand-name"]').exists()).toBe(true);
    expect(useDesignStore().draftTokens).toEqual({ colors: { primary: "#111" } });
    w.unmount();
  });

  it("Apply to site sends the exact prompt as a request", async () => {
    const { w } = await setup();
    await button(w, "design-apply").trigger("click");
    expect(useAgentOutboxStore().messages[0]).toEqual({ kind: "text", text: APPLY_PROMPT, submit: true });
    expect(toast.info).toHaveBeenCalledWith("Asked the agent to apply the design");
    w.unmount();
  });

  it("Apply is disabled while dirty", async () => {
    const { w } = await setup();
    expect(button(w, "design-apply").attributes("disabled")).toBeUndefined();
    await button(w, "body-edit").trigger("click");
    expect(button(w, "design-apply").attributes("disabled")).toBeDefined();
    await button(w, "design-apply").trigger("click");
    expect(useAgentOutboxStore().messages).toEqual([]);
    w.unmount();
  });

  it("Apply is disabled while the file does not exist yet", async () => {
    const { w } = await setup({ design: vi.fn(async () => missing()) });
    await button(w, "design-create").trigger("click");
    await flushPromises();
    expect(button(w, "design-apply").attributes("disabled")).toBeDefined();
    w.unmount();
  });

  it("invalid frontmatter shows the read-only message", async () => {
    const { ApiError } = await import("@/core/api");
    const details = [{ path: ["source"], message: "---\n: [\n---\nbody\n" }];
    const { w } = await setup({ design: vi.fn(async () => { throw new ApiError("BADREQ", "bad yaml at line 2", 400, { reason: "invalid_design", details }); }) });
    const msg = button(w, "design-invalid");
    expect(msg.text()).toContain("DESIGN.md has invalid frontmatter. Ask the agent to fix it.");
    expect(msg.text()).toContain("bad yaml at line 2");
    const source = w.get('[data-testid="design-invalid-source"]');
    expect(source.element.tagName).toBe("PRE");
    expect(source.text()).toBe("---\n: [\n---\nbody");
    expect(w.find('[data-testid="body-editor"]').exists()).toBe(false);
    expect(w.find('[data-testid="design-create"]').exists()).toBe(false);
    w.unmount();
  });

  it("Save calls saveDesign", async () => {
    const { w, fns } = await setup();
    expect(button(w, "design-save").attributes("disabled")).toBeDefined();
    await button(w, "body-edit").trigger("click");
    await button(w, "design-save").trigger("click");
    await flushPromises();
    expect(fns.saveDesign).toHaveBeenCalledWith(expect.objectContaining({
      project: "site", branch: "content/a", hash: "h1", body: "# Edited", tokens: { colors: { primary: "#000" } },
    }));
    expect(useDesignStore().dirty).toBe(false);
    w.unmount();
  });

  it("Discard puts the draft back", async () => {
    const { w } = await setup();
    expect(button(w, "design-discard").attributes("disabled")).toBeDefined();
    await button(w, "body-edit").trigger("click");
    expect(button(w, "design-discard").attributes("disabled")).toBeUndefined();
    expect(w.get('[data-testid="body-text"]').text()).toBe("# Edited");
    await button(w, "design-discard").trigger("click");
    expect(w.get('[data-testid="body-text"]').text()).toBe("# Design");
    w.unmount();
  });

  it("a save refused on main asks for a change, then saves on the branch it creates", async () => {
    const { ApiError } = await import("@/core/api");
    const saveDesign = vi.fn()
      .mockRejectedValueOnce(new ApiError("CHANGE_REQUIRED", "Start a change first", 409))
      .mockResolvedValueOnce(doc({ body: "# Edited", hash: "h2", branch: "content/new" }));
    const { w } = await setup({ saveDesign });
    await button(w, "body-edit").trigger("click");
    await button(w, "design-save").trigger("click");
    await flushPromises();
    expect(button(w, "design-needs-change").text()).toContain("Create change");
    await button(w, "design-create-change").trigger("click");
    await button(w, "new-change-stub").trigger("click");
    await flushPromises();
    expect(saveDesign).toHaveBeenLastCalledWith(expect.objectContaining({ branch: "content/new", body: "# Edited" }));
    expect(w.find('[data-testid="design-needs-change"]').exists()).toBe(false);
    expect(useDesignStore().dirty).toBe(false);
    w.unmount();
  });

  it("a conflict keeps the draft; Reload confirms, loads the file from disk and the next save sends its hash", async () => {
    const { ApiError } = await import("@/core/api");
    const saveDesign = vi.fn()
      .mockRejectedValueOnce(new ApiError("CONFLICT", "File changed", 409, { reason: "stale_content" }))
      .mockResolvedValueOnce(doc({ body: "# Edited", hash: "h3", branch: "content/b" }));
    const design = vi.fn().mockResolvedValueOnce(doc()).mockResolvedValue(doc({ body: "# On disk", hash: "h9", branch: "content/b" }));
    const { w } = await setup({ saveDesign, design });
    await button(w, "body-edit").trigger("click");
    await button(w, "design-save").trigger("click");
    await flushPromises();
    expect(button(w, "design-conflict").text()).toContain("File changed");
    expect(useDesignStore().draftBody).toBe("# Edited");
    const confirm = vi.fn(() => false);
    vi.stubGlobal("confirm", confirm);
    await button(w, "design-reload").trigger("click");
    await flushPromises();
    expect(confirm).toHaveBeenCalledWith("Discard unsaved design changes?");
    expect(useDesignStore().draftBody).toBe("# Edited");
    confirm.mockReturnValue(true);
    await button(w, "design-reload").trigger("click");
    await flushPromises();
    expect(w.get('[data-testid="body-text"]').text()).toBe("# On disk");
    expect(w.find('[data-testid="design-conflict"]').exists()).toBe(false);
    await button(w, "body-edit").trigger("click");
    await button(w, "design-save").trigger("click");
    await flushPromises();
    expect(saveDesign).toHaveBeenLastCalledWith(expect.objectContaining({ hash: "h9", branch: "content/b", body: "# Edited" }));
    vi.unstubAllGlobals();
    w.unmount();
  });

  it("the changed-on-disk banner's Reload replaces the draft with the file", async () => {
    const design = vi.fn().mockResolvedValueOnce(doc()).mockResolvedValue(doc({ body: "# On disk", hash: "h9" }));
    const { w } = await setup({ design });
    await button(w, "body-edit").trigger("click");
    await useDesignStore().refreshForBranch("content/b");
    await flushPromises();
    expect(w.find('[data-testid="design-changed-on-disk"]').exists()).toBe(true);
    vi.stubGlobal("confirm", vi.fn(() => true));
    await button(w, "design-reload").trigger("click");
    await flushPromises();
    expect(w.get('[data-testid="body-text"]').text()).toBe("# On disk");
    expect(w.find('[data-testid="design-changed-on-disk"]').exists()).toBe(false);
    vi.unstubAllGlobals();
    w.unmount();
  });

  it("blocks leaving the tab until a dirty draft is discarded", async () => {
    const { w, router } = await setup();
    await button(w, "body-edit").trigger("click");
    vi.stubGlobal("confirm", vi.fn(() => false));
    await router.push("/other");
    expect(router.currentRoute.value.path).toBe("/p/site/design");
    vi.stubGlobal("confirm", vi.fn(() => true));
    await router.push("/other");
    expect(router.currentRoute.value.path).toBe("/other");
    vi.unstubAllGlobals();
    w.unmount();
  });
});
