// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import type { SetupStatus } from "@/core/types";
import { dismissSetup, setupDismissed, setupIncomplete } from "./setup";

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
  it("is false when everything is ready", () => {
    expect(setupIncomplete(complete)).toBe(false);
  });

  it.each([
    ["git", { git: false }],
    ["gh", { gh: false }],
    ["GitHub sign-in", { githubSignedIn: false }],
    ["a chat-ready assistant", { providers: complete.providers.map((p) => ({ ...p, chatReady: false })) }],
  ])("is true without %s", (_, patch) => {
    expect(setupIncomplete({ ...complete, ...patch })).toBe(true);
  });

  it("counts chat-ready assistants rather than installed copies", () => {
    // Available but not signed in is not enough to chat.
    expect(setupIncomplete({ ...complete, providers: [{ id: "claude", name: "Claude", available: true, chatReady: false }] })).toBe(true);
    // Chat-ready is enough even when the daemon marks the copy unavailable.
    expect(setupIncomplete({ ...complete, providers: [{ id: "opencode", name: "OpenCode", available: false, chatReady: true }] })).toBe(false);
  });

  it("waits for the site builder when a daemon reports it", () => {
    const node = { phase: "downloading" as const, version: null, source: null, progress: 40, message: null };
    expect(setupIncomplete({ ...complete, node })).toBe(true);
    expect(setupIncomplete({ ...complete, node: { ...node, phase: "ready", progress: null } })).toBe(false);
  });
});

describe("setup dismissal", () => {
  it("persists across reads", () => {
    expect(setupDismissed()).toBe(false);
    dismissSetup();
    expect(setupDismissed()).toBe(true);
  });
});
