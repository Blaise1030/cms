// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import type { SetupStatus } from "@/core/types";
import { dismissSetup, onboardingStartStep, setupDismissed, setupIncomplete } from "./setup";

const complete: SetupStatus = {
  git: true,
  gh: true,
  githubSignedIn: true,
  providers: [
    { id: "cursor", name: "Cursor", available: true, chatReady: false },
    { id: "claude", name: "Claude", available: true, chatReady: true },
  ],
  projectsDir: "/p",
};

afterEach(() => localStorage.clear());

describe("setupIncomplete", () => {
  it("is false when a chat-ready assistant is present", () => {
    expect(setupIncomplete(complete)).toBe(false);
  });

  it("does not block on optional GitHub pieces", () => {
    expect(setupIncomplete({ ...complete, git: false, gh: false, githubSignedIn: false })).toBe(false);
  });

  it("is true without a chat-ready assistant", () => {
    expect(
      setupIncomplete({
        ...complete,
        providers: complete.providers.map((p) => ({ ...p, chatReady: false })),
      }),
    ).toBe(true);
  });

  it("counts chat-ready assistants rather than installed copies", () => {
    expect(
      setupIncomplete({
        ...complete,
        providers: [{ id: "claude", name: "Claude", available: true, chatReady: false }],
      }),
    ).toBe(true);
    expect(
      setupIncomplete({
        ...complete,
        providers: [{ id: "opencode", name: "OpenCode", available: false, chatReady: true }],
      }),
    ).toBe(false);
  });

  it("waits for the site builder when a daemon reports it", () => {
    const node = { phase: "downloading" as const, version: null, source: null, progress: 40, message: null };
    expect(setupIncomplete({ ...complete, node })).toBe(true);
    expect(setupIncomplete({ ...complete, node: { ...node, phase: "ready", progress: null } })).toBe(false);
    expect(setupIncomplete({ ...complete, node: { ...node, phase: "updateAvailable", progress: null } })).toBe(
      false,
    );
  });
});

describe("onboardingStartStep", () => {
  it("opens the first unfinished step", () => {
    expect(onboardingStartStep({ chatReady: false, siteReady: false })).toBe(1);
    expect(onboardingStartStep({ chatReady: true, siteReady: false })).toBe(2);
    expect(onboardingStartStep({ chatReady: true, siteReady: true })).toBe(3);
  });
});

describe("setup dismissal", () => {
  it("persists across reads", () => {
    expect(setupDismissed()).toBe(false);
    dismissSetup();
    expect(setupDismissed()).toBe(true);
  });
});
