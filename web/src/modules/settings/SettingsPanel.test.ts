// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import type { Project, SettingsDoc } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

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

import SettingsPanel from "./SettingsPanel.vue";
import { useSettingsStore } from "./store";
import { useProjectsStore } from "@/modules/projects/store";

const defaults = { root: ".", installDir: ".", install: "npm install --no-audit --no-fund --no-progress", dev: "npm run dev -- --port {port} --host {host} --ignore-lock" };
const doc = (over: Partial<SettingsDoc> = {}): SettingsDoc => ({
  exists: true, settings: { root: "apps/web" }, defaults, hash: "h1", branch: "content/a", errors: {}, ...over,
});
const project: Project = { id: "site", name: "Site", port: 4321, devserver: "ready", autoApprove: false, repo: null };

async function setup(api: Record<string, unknown> = {}) {
  h.daemon = createFakeDaemon({ settings: vi.fn(async () => doc()), saveSettings: vi.fn(async () => doc({ hash: "h2" })), ...api });
  setActivePinia(createPinia());
  useProjectsStore().projects = [project];
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/p/:id/settings", component: SettingsPanel, props: r => ({ projectId: r.params.id as string }) },
      { path: "/other", component: { template: "<p>Other</p>" } },
    ],
  });
  await router.push("/p/site/settings");
  await router.isReady();
  const w = mount(RouterView, { global: { plugins: [router] } });
  await flushPromises();
  const fns = h.daemon.api as unknown as Record<"settings" | "saveSettings", ReturnType<typeof vi.fn>>;
  return { w, router, fns };
}
type Wrapper = Awaited<ReturnType<typeof setup>>["w"];
const el = (w: Wrapper, id: string) => w.get(`[data-testid="${id}"]`);
const input = (w: Wrapper, field: string) => el(w, `settings-${field}`).element as HTMLInputElement;

beforeEach(() => { vi.clearAllMocks(); });

describe("SettingsPanel", () => {
  it("shows four inputs whose placeholders are the defaults", async () => {
    const { w } = await setup();
    expect(input(w, "root").placeholder).toBe(".");
    expect(input(w, "installDir").placeholder).toBe(".");
    expect(input(w, "install").placeholder).toBe("npm install --no-audit --no-fund --no-progress");
    expect(input(w, "dev").placeholder).toBe("npm run dev -- --port {port} --host {host} --ignore-lock");
    expect(input(w, "root").value).toBe("apps/web");
    expect(input(w, "dev").value).toBe("");
    w.unmount();
  });

  it("renders a field error under its input", async () => {
    const { w } = await setup({ settings: vi.fn(async () => doc({ errors: { installDir: "must be a relative path" } })) });
    expect(el(w, "settings-error-installDir").text()).toBe("must be a relative path");
    expect(w.find('[data-testid="settings-error-dev"]').exists()).toBe(false);
    w.unmount();
  });

  it("on main asks for a change, then saves on the branch it creates", async () => {
    const { ApiError } = await import("@/core/api");
    const saveSettings = vi.fn()
      .mockRejectedValueOnce(new ApiError("CHANGE_REQUIRED", "Start a change first", 409))
      .mockResolvedValueOnce(doc({ settings: { root: "site" }, hash: "h2", branch: "content/new" }));
    const { w } = await setup({ saveSettings });
    await el(w, "settings-root").setValue("site");
    await el(w, "settings-save").trigger("click");
    await flushPromises();
    expect(w.get('[data-testid="settings-needs-change"] p').text()).toBe("Settings can't be edited on main. Create a change.");
    await el(w, "settings-create-change").trigger("click");
    await el(w, "new-change-stub").trigger("click");
    await flushPromises();
    expect(saveSettings).toHaveBeenLastCalledWith(expect.objectContaining({ branch: "content/new", settings: { root: "site" } }));
    expect(w.find('[data-testid="settings-needs-change"]').exists()).toBe(false);
    expect(useSettingsStore().dirty).toBe(false);
    w.unmount();
  });

  it("warns when the file on disk has errors", async () => {
    const { w } = await setup({ settings: vi.fn(async () => doc({ errors: { _file: "expected value at line 1 column 1" } })) });
    expect(w.get('[data-testid="settings-file-errors"] p').text()).toBe("orso.json has errors — the dev server won't start until they're fixed.");
    expect(el(w, "settings-file-errors").text()).toContain("expected value at line 1 column 1");
    w.unmount();
  });

  it("has no file-errors banner for a valid file", async () => {
    const { w } = await setup();
    expect(w.find('[data-testid="settings-file-errors"]').exists()).toBe(false);
    w.unmount();
  });

  it("hints when an astro dev command lacks --ignore-lock", async () => {
    const { w } = await setup();
    expect(w.find('[data-testid="settings-dev-hint"]').exists()).toBe(false);
    await el(w, "settings-dev").setValue("astro dev --port {port} --host {host}");
    expect(el(w, "settings-dev-hint").text()).toContain("--ignore-lock");
    await el(w, "settings-dev").setValue("astro dev --port {port} --host {host} --ignore-lock");
    expect(w.find('[data-testid="settings-dev-hint"]').exists()).toBe(false);
    w.unmount();
  });

  it("disables Save while the dev command lacks {port}", async () => {
    const { w } = await setup();
    await el(w, "settings-dev").setValue("pnpm dev");
    expect(el(w, "settings-save").attributes("disabled")).toBeDefined();
    expect(el(w, "settings-error-dev").text()).toContain("{port}");
    await el(w, "settings-dev").setValue("pnpm dev --port {port}");
    expect(el(w, "settings-save").attributes("disabled")).toBeUndefined();
    w.unmount();
  });

  it("Save sends the draft, then shows the dev server status", async () => {
    const { w, fns } = await setup();
    expect(el(w, "settings-save").attributes("disabled")).toBeDefined();
    expect(w.find('[data-testid="settings-devserver"]').exists()).toBe(false);
    await el(w, "settings-install").setValue("pnpm install");
    await el(w, "settings-save").trigger("click");
    await flushPromises();
    expect(fns.saveSettings).toHaveBeenCalledWith({ project: "site", branch: "content/a", hash: "h1", settings: { root: "apps/web", install: "pnpm install" } });
    h.daemon.emit({ type: "devserver.status", projectId: "site", state: "starting" });
    await flushPromises();
    expect(el(w, "settings-devserver").text()).toContain("starting");
    h.daemon.emit({ type: "devserver.status", projectId: "site", state: "crashed", message: "orso.json: dev must contain {port}" });
    await flushPromises();
    expect(el(w, "settings-devserver").text()).toContain("crashed");
    expect(el(w, "settings-devserver").text()).toContain("orso.json: dev must contain {port}");
    w.unmount();
  });

  it("blocks leaving the tab until a dirty draft is discarded", async () => {
    const { w, router } = await setup();
    await el(w, "settings-dev").setValue("pnpm dev --port {port}");
    vi.stubGlobal("confirm", vi.fn(() => false));
    await router.push("/other");
    expect(router.currentRoute.value.path).toBe("/p/site/settings");
    vi.stubGlobal("confirm", vi.fn(() => true));
    await router.push("/other");
    expect(router.currentRoute.value.path).toBe("/other");
    vi.unstubAllGlobals();
    w.unmount();
  });
});
