// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import { NetworkError } from "@/core/api";
import type { SetupStatus } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import SetupCheck from "./SetupCheck.vue";

const allMissing: SetupStatus = {
  git: false,
  gh: false,
  githubSignedIn: false,
  providers: [
    { id: "cursor", name: "Cursor CLI", available: false },
    { id: "claude", name: "Claude Code", available: false },
    { id: "codex", name: "Codex", available: false },
  ],
  projectsDir: "/Users/ada/orso-projects",
};

const ready: SetupStatus = {
  ...allMissing,
  git: true,
  gh: true,
  githubSignedIn: true,
  providers: allMissing.providers.map((p) => ({ ...p, available: p.id === "claude" })),
};

const signedOut = { configured: true, deviceFlow: true, source: null, user: null };

function mountWith(setupStatus: () => Promise<SetupStatus>) {
  const api = { setupStatus: vi.fn(setupStatus), githubStatus: vi.fn(async () => signedOut) };
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  return { w: mount(SetupCheck, { attachTo: document.body }), api };
}

const row = (w: ReturnType<typeof mount>, item: string) => w.find(`[data-testid='setup-${item}']`);

enableAutoUnmount(afterEach);

describe("SetupCheck", () => {
  it("asks for git, the GitHub CLI and an agent when everything is missing", async () => {
    const { w } = mountWith(async () => allMissing);
    await flushPromises();
    for (const item of ["git", "gh", "agent"]) {
      expect(row(w, item).text()).toContain("Action needed");
      expect(row(w, item).find("a[target='_blank'][rel='noopener noreferrer']").exists()).toBe(true);
    }
    expect(row(w, "github").text()).toContain("Action needed");
    expect(w.findAll("[data-testid='setup-agent'] a").map((a) => a.text())).toEqual(
      expect.arrayContaining(["Cursor CLI", "Claude Code", "Codex"]),
    );
    expect(w.text()).toContain("/Users/ada/orso-projects");
  });

  it("shows every item ready", async () => {
    const { w } = mountWith(async () => ready);
    await flushPromises();
    for (const item of ["git", "gh", "github", "agent"]) {
      expect(row(w, item).text()).toContain("Ready");
      expect(row(w, item).text()).not.toContain("Action needed");
    }
    expect(row(w, "agent").text()).toContain("Claude Code");
  });

  it("rechecks on request", async () => {
    let next = allMissing;
    const { w, api } = mountWith(async () => next);
    await flushPromises();
    next = ready;
    await w.find("[data-testid='setup-recheck']").trigger("click");
    await flushPromises();
    expect(api.setupStatus).toHaveBeenCalledTimes(2);
    expect(row(w, "git").text()).toContain("Ready");
  });

  it("shows a friendly message, not the raw error, when the check fails", async () => {
    const { w } = mountWith(async () => {
      throw new NetworkError("cannot reach daemon at http://127.0.0.1:7800");
    });
    await flushPromises();
    expect(w.find("[data-testid='setup-error']").exists()).toBe(true);
    expect(w.text()).not.toContain("cannot reach daemon");
    expect(w.text()).not.toContain("127.0.0.1");
  });

  it("emits continue when the user moves on", async () => {
    const { w } = mountWith(async () => allMissing);
    await flushPromises();
    await w.find("[data-testid='setup-continue']").trigger("click");
    expect(w.emitted("continue")).toHaveLength(1);
  });
});
