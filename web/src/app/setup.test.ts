// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import type { SetupStatus } from "@/core/types";
import { dismissSetup, setupDismissed, setupIncomplete } from "./setup";

const complete: SetupStatus = {
  git: true,
  gh: true,
  githubSignedIn: true,
  providers: [
    { id: "cursor", name: "Cursor CLI", available: false },
    { id: "claude", name: "Claude Code", available: true },
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
    ["an agent", { providers: complete.providers.map((p) => ({ ...p, available: false })) }],
  ])("is true without %s", (_, patch) => {
    expect(setupIncomplete({ ...complete, ...patch })).toBe(true);
  });
});

describe("setup dismissal", () => {
  it("persists across reads", () => {
    expect(setupDismissed()).toBe(false);
    dismissSetup();
    expect(setupDismissed()).toBe(true);
  });
});
