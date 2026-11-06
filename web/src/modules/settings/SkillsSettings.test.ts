// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import SkillsSettings from "./SkillsSettings.vue";

let wrapper: VueWrapper | null = null;

async function setup(api: Record<string, unknown> = {}) {
  h.daemon = createFakeDaemon({
    skillsSettings: vi.fn(async () => ({
      globalPath: "/Users/ada/global-skills",
      defaultGlobalPath: "/Users/ada/.orso/global-skills",
    })),
    saveSkillsSettings: vi.fn(async (body: { globalPath: string }) => ({
      globalPath: body.globalPath || "/Users/ada/.orso/global-skills",
      defaultGlobalPath: "/Users/ada/.orso/global-skills",
    })),
    ...api,
  });
  setActivePinia(createPinia());
  return h.daemon.api as unknown as Record<string, ReturnType<typeof vi.fn>>;
}

function mountSettings() {
  wrapper = mount(SkillsSettings, { attachTo: document.body });
  return wrapper;
}

const pathInput = () => wrapper!.get<HTMLInputElement>('[data-testid="skills-settings-path"]');

beforeEach(() => {
  vi.restoreAllMocks();
  document.body.innerHTML = "";
});

afterEach(() => {
  wrapper?.unmount();
  wrapper = null;
  document.body.innerHTML = "";
});

describe("SkillsSettings", () => {
  it("loads the global path and Reset to default saves an empty path", async () => {
    const fns = await setup();
    mountSettings();
    await flushPromises();

    expect(pathInput().element.value).toBe("/Users/ada/global-skills");

    await wrapper!.get('[data-testid="skills-settings-reset"]').trigger("click");
    await flushPromises();

    expect(fns.saveSkillsSettings).toHaveBeenCalledWith({ globalPath: "" });
    expect(pathInput().element.value).toBe("/Users/ada/.orso/global-skills");
  });

  it("Save persists the entered path", async () => {
    const fns = await setup();
    mountSettings();
    await flushPromises();

    await pathInput().setValue("/custom/path");
    await wrapper!.get('[data-testid="skills-settings-save"]').trigger("click");
    await flushPromises();

    expect(fns.saveSkillsSettings).toHaveBeenCalledWith({ globalPath: "/custom/path" });
  });
});
