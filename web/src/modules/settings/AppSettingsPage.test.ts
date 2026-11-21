// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createMemoryHistory, createRouter, type Router } from "vue-router";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import type { ProviderSignInState, RuntimeStatus, SetupProvider, SetupStatus } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import { SelectRoot } from "reka-ui";
import { useTypingSound } from "@/lib/typingSound";
import AppSettingsPage from "./AppSettingsPage.vue";
import { queryPlugin } from "@/test/queryClient";

const runtime = (over: Partial<RuntimeStatus> = {}): RuntimeStatus => ({
  phase: "ready",
  version: "2.2.5",
  source: "managed",
  progress: null,
  message: null,
  downloadBytes: 157_286_400,
  ...over,
});

const provider = (id: string, over: Partial<SetupProvider> = {}): SetupProvider => ({
  id,
  name: id[0]!.toUpperCase() + id.slice(1),
  available: true,
  runtime: runtime(),
  installed: true,
  signedIn: true,
  chatReady: true,
  freeModels: false,
  reason: null,
  ...over,
});

const empty = { render: () => null };
let router: Router;

function mountAt(path = "/settings", api: Record<string, unknown> = {}) {
  h.daemon = createFakeDaemon({
    health: vi.fn(async () => ({ status: "ok", version: "0.1.0" })),
    setupStatus: vi.fn(async (): Promise<SetupStatus> => ({
      git: true,
      gh: true,
      githubSignedIn: false,
      node: runtime({ version: "24.0.0" }),
      providers: [
        provider("opencode", { freeModels: true, signedIn: null }),
        provider("codex", {
          available: false,
          installed: false,
          runtime: runtime({ phase: "notDownloaded", version: null, source: null }),
          signedIn: null,
          chatReady: false,
        }),
      ],
      projectsDir: "/Users/ada/orso-projects",
    })),
    setupRuntimes: vi.fn(async () => ({ node: runtime({ version: "24.0.0" }), providers: [] })),
    githubStatus: vi.fn(async () => ({
      configured: true,
      deviceFlow: true,
      source: null,
      user: null,
    })),
    ...api,
  });
  const pinia = createPinia();
  setActivePinia(pinia);
  router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/", component: empty },
      { path: "/settings", component: empty },
      { path: "/p/:id", component: empty, children: [{ path: ":rest(.*)*", component: empty }] },
    ],
  });
  void router.push(path);
  return { router, pinia };
}

async function mountPage(path = "/settings", api: Record<string, unknown> = {}) {
  const { router, pinia } = mountAt(path, api);
  await router.isReady();
  const w = mount(AppSettingsPage, {
    attachTo: document.body,
    global: { plugins: [pinia, router, queryPlugin().plugin] },
  });
  await flushPromises();
  return { w, router };
}

async function openTab(w: Awaited<ReturnType<typeof mountPage>>["w"], tab: string) {
  await w.get(`[data-testid='app-settings-tab-${tab}']`).trigger("click");
  await flushPromises();
}

async function search(
  w: Awaited<ReturnType<typeof mountPage>>["w"],
  value: string,
) {
  await w.get("[data-testid='app-settings-search']").setValue(value);
  await flushPromises();
}

enableAutoUnmount(afterEach);

beforeEach(() => {
  localStorage.clear();
  document.documentElement.classList.remove("dark");
  delete document.documentElement.dataset.desktop;
  vi.stubGlobal("matchMedia", () => ({
    matches: false,
    addEventListener() {},
    removeEventListener() {},
  }));
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("AppSettingsPage", () => {
  it("renders the sidebar groups, search and GitHub account", async () => {
    const { w } = await mountPage();
    expect(w.get("[data-testid='app-settings-page']").text()).toContain("Settings");
    expect(w.find("[data-testid='app-settings-sidebar']").exists()).toBe(true);
    expect(w.get("[data-testid='app-settings-search']").attributes("placeholder")).toBe(
      "Find a setting",
    );
    expect(w.get("[data-testid='app-settings-search-hint']").text()).toContain("⌘F");
    expect(w.get("[data-testid='app-settings-nav']").text()).toContain("Setup");
    expect(w.get("[data-testid='app-settings-nav']").text()).toContain("Preferences");
    expect(w.findAll("[data-testid^='app-settings-tab-']")).toHaveLength(5);
    expect(w.find("[data-testid='app-settings-tab-about']").exists()).toBe(true);
    expect(w.find("[data-testid='app-settings-account']").exists()).toBe(true);
  });

  it("adds room for the traffic lights on the macOS desktop", async () => {
    document.documentElement.dataset.desktop = "macos";
    const { w } = await mountPage();
    expect(w.get("[data-testid='app-settings-nav']").classes()).toContain("pt-11");
  });

  it("defaults to the Assistant section with its nav row active", async () => {
    const { w, router } = await mountPage();
    expect(router.currentRoute.value.query.tab).toBeUndefined();
    expect(w.find("[data-testid='assistant-settings']").exists()).toBe(true);
    expect(w.get("[data-testid='app-settings-tab-assistant']").attributes("data-active")).toBe(
      "true",
    );
    expect(w.get("[data-testid='app-settings-tab-assistant']").text()).toContain("Assistant");
  });

  it("loads each section and records it in the query", async () => {
    const { w, router } = await mountPage();

    await openTab(w, "builder");
    expect(router.currentRoute.value.query.tab).toBe("builder");
    expect(w.find("[data-testid='site-builder-settings']").exists()).toBe(true);

    await openTab(w, "github");
    expect(router.currentRoute.value.query.tab).toBe("github");
    expect(w.find("[data-testid='github-settings']").exists()).toBe(true);

    await openTab(w, "appearance");
    expect(router.currentRoute.value.query.tab).toBe("appearance");
    expect(w.find("[data-testid='appearance-settings']").exists()).toBe(true);

    await openTab(w, "about");
    expect(router.currentRoute.value.query.tab).toBe("about");
    expect(w.find("[data-testid='about-settings']").exists()).toBe(true);

    await openTab(w, "assistant");
    expect(router.currentRoute.value.query.tab).toBe("assistant");
    expect(w.find("[data-testid='assistant-settings']").exists()).toBe(true);
  });

  it("renders every section in one list and scrolls to one from the sidebar", async () => {
    const scrolled: string[] = [];
    HTMLElement.prototype.scrollIntoView = function (this: HTMLElement) {
      scrolled.push(this.dataset.testid ?? "");
    };
    const { w } = await mountPage();
    for (const id of ["assistant", "builder", "github", "appearance", "about"]) {
      expect(w.find(`[data-testid='app-settings-section-${id}']`).exists()).toBe(true);
    }

    await openTab(w, "appearance");
    expect(scrolled.at(-1)).toBe("app-settings-section-appearance");
    expect(w.get("[data-testid='app-settings-tab-appearance']").attributes("data-active")).toBe(
      "true",
    );
  });

  it("falls back to Assistant for an unknown tab and deep links to a known one", async () => {
    const { w } = await mountPage("/settings?tab=builder");
    expect(w.find("[data-testid='site-builder-settings']").exists()).toBe(true);

    const { w: w2 } = await mountPage("/settings?tab=nope");
    expect(w2.find("[data-testid='assistant-settings']").exists()).toBe(true);
  });

  it("filters the nav by label and Enter opens the first match", async () => {
    const { w, router } = await mountPage();

    await search(w, "appear");
    expect(w.find("[data-testid='app-settings-tab-assistant']").exists()).toBe(false);
    expect(w.find("[data-testid='app-settings-tab-appearance']").exists()).toBe(true);
    expect(w.find("[data-testid='app-settings-tab-about']").exists()).toBe(false);

    await w.get("[data-testid='app-settings-search']").trigger("keydown.enter");
    await flushPromises();
    expect(router.currentRoute.value.query.tab).toBe("appearance");
    expect(w.find("[data-testid='appearance-settings']").exists()).toBe(true);
  });

  it("shows a quiet empty line when no setting matches", async () => {
    const { w } = await mountPage();
    await search(w, "nothing-like-this");
    expect(w.find("[data-testid='app-settings-tab-assistant']").exists()).toBe(false);
    expect(w.find("[data-testid='app-settings-tab-about']").exists()).toBe(false);
    expect(w.get("[data-testid='app-settings-search-empty']").text()).toBe("No settings found.");
  });

  it("focuses the search field with Cmd/Ctrl+F", async () => {
    const { w } = await mountPage();
    const input = w.get("[data-testid='app-settings-search']");
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "f", metaKey: true }));
    await flushPromises();
    expect(document.activeElement).toBe(input.element);
  });

  it("goes home from Back when there is no previous page", async () => {
    const { w, router } = await mountPage();
    await w.get("[data-testid='app-settings-back']").trigger("click");
    await flushPromises();
    expect(router.currentRoute.value.path).toBe("/");
  });

  it("goes back to the previous page when the history has one", async () => {
    const { router, pinia } = mountAt();
    await router.isReady();
    await router.push("/");
    await router.push({ path: "/settings", state: { back: "/" } });
    const w = mount(AppSettingsPage, { global: { plugins: [pinia, router, queryPlugin().plugin] } });
    await flushPromises();
    await w.get("[data-testid='app-settings-back']").trigger("click");
    await flushPromises();
    expect(router.currentRoute.value.path).toBe("/");
  });
});

describe("Project rail", () => {
  async function openFrom(back: string) {
    const { router, pinia } = mountAt();
    await router.isReady();
    await router.push(back);
    await router.push({ path: "/settings", state: { back } });
    const w = mount(AppSettingsPage, { global: { plugins: [pinia, router, queryPlugin().plugin] } });
    await flushPromises();
    return w;
  }

  it("shows the rail of the project Settings was opened from, with App settings current", async () => {
    const w = await openFrom("/p/my%20site/content/blog");
    expect(w.get("[data-testid='workspace-tab-code']").attributes("href")).toBe("/p/my%20site/code");
    expect(w.get("[data-testid='project-rail-app-settings']").attributes("aria-current")).toBe("page");
  });

  it("has no rail when opened from the projects list", async () => {
    const w = await openFrom("/");
    expect(w.find("[data-testid='project-rail']").exists()).toBe(false);
  });
});

describe("Assistant settings", () => {
  it("renders the Assistants section and handles the row actions", async () => {
    const download = vi.fn(async () => runtime());
    const { w } = await mountPage("/settings", { downloadRuntime: download });
    expect(w.get("[data-testid='assistant-settings']").text()).toContain("Assistants");
    expect(w.find("[data-testid='assistant-row-opencode']").exists()).toBe(true);
    expect(w.get("[data-testid='assistant-row-codex']").attributes("data-action")).toBe(
      "download",
    );

    await w.get("[data-testid='assistant-action-codex']").trigger("click");
    await flushPromises();
    expect(download).toHaveBeenCalledWith("codex");
  });

  it("shows the sign-in panel under a row that needs a code", async () => {
    const signIn = vi.fn(
      async (): Promise<ProviderSignInState> => ({ state: "code", message: "Enter the code" }),
    );
    const { w } = await mountPage("/settings", {
      signInProvider: signIn,
      setupStatus: vi.fn(async (): Promise<SetupStatus> => ({
        git: true,
        gh: true,
        githubSignedIn: false,
        node: runtime({ version: "24.0.0" }),
        providers: [
          provider("claude", {
            signedIn: false,
            chatReady: false,
            runtime: runtime(),
          }),
        ],
        projectsDir: "/Users/ada/orso-projects",
      })),
    });
    await w.get("[data-testid='assistant-action-claude']").trigger("click");
    await flushPromises();
    expect(signIn).toHaveBeenCalledWith("claude");
    expect(w.get("[data-testid='assistant-sign-in']").attributes("data-state")).toBe("code");
  });
});

describe("Assistant order", () => {
  const four = (assistantOrder?: string[]) =>
    vi.fn(async (): Promise<SetupStatus> => ({
      git: true,
      gh: true,
      githubSignedIn: false,
      node: runtime({ version: "24.0.0" }),
      providers: [
        provider("opencode", { freeModels: true, signedIn: null }),
        provider("codex", { signedIn: false, chatReady: false }),
        provider("claude"),
        provider("cursor"),
      ],
      projectsDir: "/Users/ada/orso-projects",
      assistantOrder,
    }));
  const order = (w: Awaited<ReturnType<typeof mountPage>>["w"]) =>
    w
      .get("[data-testid='assistant-settings-rows']")
      .findAll("[data-testid^='assistant-row-']")
      .map((row) => row.attributes("data-testid")!.replace("assistant-row-", ""));

  it("lists the saved order and marks the first connected assistant Default", async () => {
    const { w } = await mountPage("/settings", { setupStatus: four(["codex", "claude"]) });
    // Saved ids first, the rest after in the default order.
    expect(order(w)).toEqual(["codex", "claude", "opencode", "cursor"]);
    // Codex needs a sign-in, so Claude is the one new chats start with.
    expect(w.findAll("[data-testid^='assistant-default-']").map((b) => b.attributes("data-testid"))).toEqual([
      "assistant-default-claude",
    ]);
    expect(w.find("[data-testid='assistant-recommended']").exists()).toBe(true);
    expect(w.find("[data-testid='assistant-handle-claude']").exists()).toBe(true);
  });

  it("marks the recommended assistant Default until the user reorders", async () => {
    const { w } = await mountPage("/settings", { setupStatus: four() });
    expect(order(w)).toEqual(["opencode", "codex", "claude", "cursor"]);
    expect(w.find("[data-testid='assistant-default-opencode']").exists()).toBe(true);
  });

  it("moves a row with Alt+Up, saves the order and keeps focus on its handle", async () => {
    const save = vi.fn(async (order: string[]) => ({ order }));
    const { w } = await mountPage("/settings", { setupStatus: four(), saveAssistantOrder: save });
    const handle = w.get("[data-testid='assistant-handle-claude']");
    // Without Alt the arrows do nothing.
    await handle.trigger("keydown", { key: "ArrowUp" });
    expect(save).not.toHaveBeenCalled();
    await handle.trigger("keydown", { key: "ArrowUp", altKey: true });
    await handle.trigger("keydown", { key: "ArrowUp", altKey: true });
    await flushPromises();
    expect(save).toHaveBeenLastCalledWith(["claude", "opencode", "codex", "cursor"]);
    expect(order(w)).toEqual(["claude", "opencode", "codex", "cursor"]);
    expect(w.find("[data-testid='assistant-default-claude']").exists()).toBe(true);
    await new Promise((resolve) => requestAnimationFrame(resolve));
    expect(document.activeElement?.getAttribute("data-testid")).toBe("assistant-handle-claude");
  });

  it("reorders live while dragging, saves on drop and restores a cancelled drag", async () => {
    const save = vi.fn(async (order: string[]) => ({ order }));
    const { w } = await mountPage("/settings", { setupStatus: four(), saveAssistantOrder: save });
    const transfer = (dropEffect: string) => ({ setData() {}, effectAllowed: "", dropEffect });

    await w.get("[data-testid='assistant-row-cursor']").trigger("dragstart", { dataTransfer: transfer("move") });
    await w.get("[data-testid='assistant-row-opencode']").trigger("dragover", { dataTransfer: transfer("move") });
    expect(order(w)).toEqual(["cursor", "opencode", "codex", "claude"]);
    await w.get("[data-testid='assistant-row-cursor']").trigger("dragend", { dataTransfer: transfer("none") });
    expect(order(w)).toEqual(["opencode", "codex", "claude", "cursor"]);
    expect(save).not.toHaveBeenCalled();

    await w.get("[data-testid='assistant-row-cursor']").trigger("dragstart", { dataTransfer: transfer("move") });
    await w.get("[data-testid='assistant-row-opencode']").trigger("dragover", { dataTransfer: transfer("move") });
    await w.get("[data-testid='assistant-row-cursor']").trigger("dragend", { dataTransfer: transfer("move") });
    await flushPromises();
    expect(save).toHaveBeenCalledWith(["cursor", "opencode", "codex", "claude"]);
    expect(order(w)).toEqual(["cursor", "opencode", "codex", "claude"]);
    expect(w.find("[data-testid='assistant-default-cursor']").exists()).toBe(true);
  });

  it("puts the order back when the daemon can't save it", async () => {
    const save = vi.fn(async () => {
      throw new Error("disk full");
    });
    const { w } = await mountPage("/settings", { setupStatus: four(), saveAssistantOrder: save });
    await w.get("[data-testid='assistant-handle-cursor']").trigger("keydown", { key: "ArrowUp", altKey: true });
    await flushPromises();
    expect(save).toHaveBeenCalled();
    expect(order(w)).toEqual(["opencode", "codex", "claude", "cursor"]);
  });
});

describe("Site builder settings", () => {
  it("shows the Package section and the Node row with its version", async () => {
    const { w } = await mountPage();
    await openTab(w, "builder");
    const section = w.get("[data-testid='site-builder-settings']");
    expect(section.text()).toContain("Package");
    expect(section.text()).toContain("Lets orso build and preview your site.");
    expect(w.find("[data-testid='assistant-row-node']").exists()).toBe(true);
    expect(w.get("[data-testid='assistant-version-node']").text()).toContain("24.0.0");
  });
});

describe("GitHub settings", () => {
  it("lists Git, the GitHub tools and the account in one Account card", async () => {
    const start = vi.fn(async () => ({
      userCode: "ABCD-1234",
      verificationUri: "https://github.com/login/device",
      expiresIn: 900,
      interval: 5,
    }));
    const { w } = await mountPage("/settings", {
      githubDeviceStart: start,
      githubDevicePoll: vi.fn(),
    });
    await openTab(w, "github");
    expect(w.get("[data-testid='github-settings']").text()).toContain("Account");
    expect(w.get("[data-testid='github-settings-git']").text()).toContain("Ready");
    expect(w.get("[data-testid='github-settings-tools']").text()).toContain("GitHub tools");
    const account = w.get("[data-testid='github-settings-account']");
    expect(account.text()).toContain("GitHub account");
    expect(account.text()).toContain("Optional");

    await w.get("[data-testid='github-settings-sign-in']").trigger("click");
    await flushPromises();
    expect(start).toHaveBeenCalled();
  });

  it("keeps the cached setup status when revisiting the GitHub tab", async () => {
    const setupStatus = vi.fn(async (): Promise<SetupStatus> => ({
      git: true,
      gh: true,
      githubSignedIn: false,
      node: runtime({ version: "24.0.0" }),
      providers: [],
      projectsDir: "/Users/ada/orso-projects",
    }));
    const { w } = await mountPage("/settings", { setupStatus });
    await openTab(w, "github");
    expect(w.find("[data-testid='github-settings-git']").exists()).toBe(true);
    const callsAfterFirstVisit = setupStatus.mock.calls.length;
    expect(callsAfterFirstVisit).toBeGreaterThan(0);

    await openTab(w, "appearance");
    await openTab(w, "github");
    expect(w.find("[data-testid='github-settings-loading']").exists()).toBe(false);
    expect(w.find("[data-testid='github-settings-git']").exists()).toBe(true);
    expect(setupStatus).toHaveBeenCalledTimes(callsAfterFirstVisit);
  });
});

describe("Appearance settings", () => {
  it("changes the theme with the visual choices", async () => {
    const { w } = await mountPage();
    await openTab(w, "appearance");
    expect(w.get("[data-testid='appearance-settings']").text()).toContain("Light and dark");
    expect(w.get("[data-testid='appearance-settings']").text()).toContain("Typing");
    expect(w.get("[data-testid='appearance-theme-light']").attributes("aria-pressed")).toBe(
      "false",
    );

    await w.get("[data-testid='appearance-theme-dark']").trigger("click");
    await flushPromises();
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(localStorage.getItem("orso.theme")).toBe("dark");
    expect(w.get("[data-testid='appearance-theme-dark']").attributes("aria-pressed")).toBe("true");

    await w.get("[data-testid='appearance-theme-system']").trigger("click");
    await flushPromises();
    expect(w.get("[data-testid='appearance-theme-system']").attributes("aria-pressed")).toBe(
      "true",
    );
    expect(localStorage.getItem("orso.theme")).toBeNull();
  });

  it("chooses a typing sound from the select", async () => {
    const { w } = await mountPage();
    await openTab(w, "appearance");
    const selects = w.findAllComponents(SelectRoot);
    const soundSelect = selects.find((s) =>
      s.find("[data-testid='appearance-typing-sound']").exists(),
    );
    expect(soundSelect).toBeTruthy();

    soundSelect!.vm.$emit("update:model-value", "mx-blue");
    await flushPromises();
    expect(useTypingSound().setting.value).toBe("mx-blue");
    expect(localStorage.getItem("orso.typingSound")).toBe("mx-blue");
  });

  it("toggles showing assistant steps, off by default", async () => {
    const { w } = await mountPage();
    await openTab(w, "appearance");
    expect(w.get("[data-testid='appearance-settings']").text()).toContain("Chat");
    const toggle = w.get("[data-testid='appearance-show-steps']");
    expect(toggle.attributes("aria-checked") ?? toggle.attributes("data-state")).toMatch(/false|unchecked/);
    expect(localStorage.getItem("orso.chat.showSteps")).toBeNull();

    await toggle.trigger("click");
    await flushPromises();
    expect(localStorage.getItem("orso.chat.showSteps")).toBe("1");
  });

  it("switches the agent panel between sidebar and floating", async () => {
    const { w } = await mountPage();
    await openTab(w, "appearance");
    expect(w.get("[data-testid='appearance-settings']").text()).toContain("Agent panel");
    expect(localStorage.getItem("orso.chat.panelLayout")).toBeNull();

    const selects = w.findAllComponents(SelectRoot);
    const panelSelect = selects.find((s) =>
      s.find("[data-testid='appearance-panel-layout']").exists(),
    );
    expect(panelSelect).toBeTruthy();
    panelSelect!.vm.$emit("update:model-value", "floating");
    await flushPromises();
    expect(localStorage.getItem("orso.chat.panelLayout")).toBe("floating");

    panelSelect!.vm.$emit("update:model-value", "sidebar");
    await flushPromises();
    expect(localStorage.getItem("orso.chat.panelLayout")).toBeNull();
  });
});

describe("About settings", () => {
  it("shows the running version and the local-first note", async () => {
    const { w } = await mountPage();
    await openTab(w, "about");
    const section = w.get("[data-testid='about-settings']");
    expect(section.text()).toContain("About");
    expect(w.get("[data-testid='about-version']").text()).toBe("v0.1.0");
    expect(section.text()).toContain("Your data stays local");
    expect(section.text()).toContain("Sites, content, and history live on this Mac.");
  });
});
