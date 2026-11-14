// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import ChangesBranchPicker from "./ChangesBranchPicker.vue";
import { useChangesStore } from "./store";

const h = vi.hoisted(() => ({
  daemon: null as unknown as FakeDaemon,
  toastError: vi.fn(),
}));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("vue-sonner", () => ({ toast: { error: h.toastError, success: vi.fn(), info: vi.fn() } }));

afterEach(() => {
  document.body.innerHTML = "";
});

function mountPicker(opts: { active?: { branch: string; title: string } | null } = {}) {
  h.daemon = createFakeDaemon({
    listChanges: vi.fn(async () => []),
  });
  setActivePinia(createPinia());
  const store = useChangesStore();
  store.active = opts.active === undefined ? { branch: "content/lw", title: "LW" } : opts.active;
  const wrapper = mount(ChangesBranchPicker, {
    props: { projectId: "site" },
    attachTo: document.body,
  });
  return { store, wrapper };
}

async function openMenu(wrapper: ReturnType<typeof mount>) {
  await wrapper.get('[data-testid="changes-menu"]').trigger("click");
  await flushPromises();
}

describe("ChangesBranchPicker", () => {
  it("always offers Live site in the menu", async () => {
    const { wrapper } = mountPicker();
    await openMenu(wrapper);
    const live = document.body.querySelector('[data-testid="live-site"]');
    expect(live?.textContent).toContain("Live site");
    expect(live?.textContent).toContain("main");
  });

  it("returns to the live site when Live site is chosen", async () => {
    const { store, wrapper } = mountPicker();
    const goLive = vi.spyOn(store, "goLive").mockResolvedValue();
    await openMenu(wrapper);
    document.body.querySelector<HTMLElement>('[data-testid="live-site"]')!.click();
    await flushPromises();
    expect(goLive).toHaveBeenCalledWith("site");
  });

  it("marks Live site selected when there is no active change", async () => {
    const { wrapper } = mountPicker({ active: null });
    expect(wrapper.get('[data-testid="active-change"]').text()).toBe("main");
    await openMenu(wrapper);
    const check = document.body.querySelector('[data-testid="live-site"] svg');
    expect(check?.classList.contains("opacity-100")).toBe(true);
  });

  it("uses a command search field to filter changes", async () => {
    const { wrapper } = mountPicker();
    await openMenu(wrapper);
    const search = document.body.querySelector<HTMLInputElement>('[data-testid="changes-search"]');
    expect(search).toBeTruthy();
    expect(search?.placeholder).toBe("Search changes…");
    expect(document.body.querySelector('[data-slot="command"]')).toBeTruthy();
  });

  it("creates a change through the API instead of asking the agent", async () => {
    const createChange = vi.fn(async () => ({
      branch: "content/seo-audit",
      title: "SEO audit",
      lastEdited: null,
    }));
    h.daemon = createFakeDaemon({
      listChanges: vi.fn(async () => []),
      createChange,
      tree: vi.fn(async () => ({ entries: [] })),
      getActiveChange: vi.fn(async () => ({
        branch: "content/seo-audit",
        title: "SEO audit",
        lastEdited: null,
      })),
      listChats: vi.fn(async () => []),
      listProviders: vi.fn(async () => ({ providers: [] })),
    });
    setActivePinia(createPinia());
    useChangesStore().active = { branch: "main", title: "main" };
    const wrapper = mount(ChangesBranchPicker, {
      props: { projectId: "site" },
      attachTo: document.body,
    });
    await openMenu(wrapper);
    document.body.querySelector<HTMLElement>('[data-testid="new-change"]')!.click();
    await flushPromises();

    const input = document.body.querySelector("[role=dialog] input") as HTMLInputElement;
    expect(input).toBeTruthy();
    input.value = "SEO audit";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await flushPromises();
    (document.body.querySelector("[role=dialog] button[type=submit]") as HTMLButtonElement).click();
    await flushPromises();

    expect(createChange).toHaveBeenCalledWith("site", "SEO audit");
    wrapper.unmount();
  });
});
