// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import type { RuntimeStatus, SetupProvider } from "@/core/types";
import ProviderRow from "./ProviderRow.vue";
import { deriveRow, LABELS, PROVIDER_NAMES } from "./rows";

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
  name: PROVIDER_NAMES[id] ?? id,
  available: true,
  runtime: runtime(),
  installed: true,
  signedIn: true,
  chatReady: true,
  freeModels: false,
  reason: null,
  ...over,
});

function render(p: SetupProvider) {
  return mount(ProviderRow, { props: { row: deriveRow(p) } });
}

const action = (w: ReturnType<typeof render>) => w.get("[data-testid='assistant-action-opencode']");

describe("ProviderRow phases", () => {
  it("offers Download before the first download", () => {
    const w = render(
      provider("opencode", {
        runtime: runtime({ phase: "notDownloaded", version: null, source: null }),
        installed: false,
        chatReady: false,
        freeModels: true,
        signedIn: null,
      }),
    );

    expect(w.text()).toContain("OpenCode");
    expect(w.text()).toContain("Free models, no account needed.");
    expect(w.text()).toContain("Recommended");
    expect(w.get("[data-testid='assistant-size-opencode']").text()).toBe("About 150 MB");
    expect(w.get("[data-testid='assistant-action-opencode']").text()).toBe(LABELS.download);
    expect(w.find("[data-testid='assistant-status-opencode']").exists()).toBe(false);
    expect(w.find("[data-testid='assistant-version-opencode']").exists()).toBe(false);
    expect(w.attributes("data-phase")).toBe("notDownloaded");
  });

  it("shows the percentage, a progress bar and Cancel while downloading", async () => {
    const w = render(
      provider("opencode", {
        runtime: runtime({ phase: "downloading", progress: 42, version: null }),
        chatReady: false,
      }),
    );

    expect(w.get("[data-testid='assistant-progress-opencode']").text()).toContain("42%");
    expect(w.find("[data-testid='assistant-progress-opencode'] [data-slot='progress']").exists()).toBe(
      true,
    );
    expect(w.get("[data-testid='assistant-action-opencode']").text()).toBe(LABELS.cancel);

    await action(w).trigger("click");
    expect(w.emitted("action")).toEqual([["cancel"]]);
  });

  it("waits on Setting up… while finishing", () => {
    const w = render(
      provider("opencode", { runtime: runtime({ phase: "finishing" }), chatReady: false }),
    );

    expect(w.get("[data-testid='assistant-status-opencode']").text()).toBe(LABELS.settingUp);
    expect(w.find("[data-testid='assistant-status-opencode'] [role='status']").exists()).toBe(true);
    expect(w.find("[data-testid='assistant-action-opencode']").exists()).toBe(false);
    expect(w.attributes("data-action")).toBe("none");
  });

  it("explains a failed download with one sentence and details, then offers Retry", async () => {
    const w = render(
      provider("opencode", {
        runtime: runtime({ phase: "downloadError", message: "The connection dropped.", version: null }),
        chatReady: false,
      }),
    );

    expect(w.get("[data-testid='assistant-status-opencode']").text()).toBe(LABELS.downloadFailed);
    const error = w.get("[data-testid='assistant-error-opencode']");
    expect(error.text()).toContain(
      "We couldn't download OpenCode. Check your connection and try again.",
    );
    expect(error.get("details summary").text()).toBe("Details");
    expect(error.get("details").text()).toContain("The connection dropped.");
    expect(w.get("[data-testid='assistant-action-opencode']").text()).toBe(LABELS.retry);

    await action(w).trigger("click");
    expect(w.emitted("action")).toEqual([["retry"]]);
  });

  it("shows Connected and Connect once the assistant is signed in", async () => {
    const w = render(provider("claude"));

    expect(w.get("[data-testid='assistant-status-claude']").text()).toBe(LABELS.connected);
    expect(w.get("[data-testid='assistant-action-claude']").text()).toBe(LABELS.connect);

    await w.get("[data-testid='assistant-action-claude']").trigger("click");
    expect(w.emitted("action")).toEqual([["connect"]]);
  });

  it("asks to sign in, and Connect starts the sign-in flow", async () => {
    const w = render(
      provider("claude", { signedIn: false, chatReady: false }),
    );

    expect(w.get("[data-testid='assistant-status-claude']").text()).toBe(LABELS.signInRequired);
    expect(w.get("[data-testid='assistant-action-claude']").text()).toBe(LABELS.connect);

    await w.get("[data-testid='assistant-action-claude']").trigger("click");
    expect(w.emitted("action")).toEqual([["signIn"]]);
  });

  it("keeps OpenCode connected on free models with an optional Sign in", async () => {
    const w = render(
      provider("opencode", { signedIn: null, chatReady: true, freeModels: true }),
    );

    expect(w.get("[data-testid='assistant-status-opencode']").text()).toBe(LABELS.connected);
    expect(w.text()).not.toContain(LABELS.signInRequired);
    expect(w.get("[data-testid='assistant-action-opencode']").text()).toBe(LABELS.signIn);
  });

  it("offers Update when a newer version is available", async () => {
    const w = render(
      provider("cursor", {
        runtime: runtime({ phase: "updateAvailable", version: "2026.09.28" }),
        signedIn: false,
      }),
    );

    expect(w.get("[data-testid='assistant-status-cursor']").text()).toBe(LABELS.updateAvailable);
    expect(w.get("[data-testid='assistant-action-cursor']").text()).toBe(LABELS.update);
    expect(w.get("[data-testid='assistant-version-cursor']").text()).toContain("2026.09.28");

    await w.get("[data-testid='assistant-action-cursor']").trigger("click");
    expect(w.emitted("action")).toEqual([["update"]]);
  });
});

describe("ProviderRow details", () => {
  it("shows the version and where a ready copy came from", () => {
    const managed = render(provider("opencode"));
    expect(managed.get("[data-testid='assistant-version-opencode']").text()).toBe(
      "Version 2.2.5 · Downloaded by orso",
    );

    const system = render(
      provider("claude", { runtime: runtime({ source: "system", version: "2.1.263" }) }),
    );
    expect(system.get("[data-testid='assistant-version-claude']").text()).toBe(
      "Version 2.1.263 · Already on your Mac",
    );
  });

  it("leaves out a size the daemon did not report", () => {
    const w = render(
      provider("opencode", {
        runtime: runtime({ phase: "notDownloaded", version: null, source: null, downloadBytes: undefined }),
      }),
    );
    expect(w.find("[data-testid='assistant-size-opencode']").exists()).toBe(false);
  });

  it("labels every action for screen readers", () => {
    const download = render(
      provider("opencode", {
        runtime: runtime({ phase: "notDownloaded", version: null, source: null }),
      }),
    );
    expect(download.get("[data-testid='assistant-action-opencode']").attributes("aria-label")).toBe(
      "Download OpenCode",
    );

    const downloading = render(
      provider("opencode", { runtime: runtime({ phase: "downloading", progress: 10 }) }),
    );
    expect(
      downloading.get("[data-testid='assistant-action-opencode']").attributes("aria-label"),
    ).toBe("Cancel downloading OpenCode");

    const signIn = render(provider("claude", { signedIn: false, chatReady: false }));
    expect(signIn.get("[data-testid='assistant-action-claude']").attributes("aria-label")).toBe(
      "Connect Claude",
    );

    const freeModels = render(
      provider("opencode", { signedIn: null, chatReady: true, freeModels: true }),
    );
    expect(
      freeModels.get("[data-testid='assistant-action-opencode']").attributes("aria-label"),
    ).toBe("Sign in to OpenCode for more models");
  });

  it("disables the action while the host is busy", () => {
    const w = mount(ProviderRow, { props: { row: deriveRow(provider("opencode")), busy: true } });
    expect(w.get("[data-testid='assistant-action-opencode']").attributes("disabled")).toBeDefined();
  });

  it("hides the dead Connect action on a connected settings row", () => {
    const w = mount(ProviderRow, {
      props: { row: deriveRow(provider("opencode")), hideConnectedAction: true },
    });
    expect(w.find("[data-testid='assistant-action-opencode']").exists()).toBe(false);
  });

  it("never uses developer words", () => {
    const rows = [
      provider("opencode", {
        runtime: runtime({ phase: "downloadError", message: "The connection dropped." }),
        chatReady: false,
      }),
      provider("codex", { runtime: runtime({ phase: "downloading", progress: 12 }) }),
      provider("claude", { runtime: runtime({ phase: "finishing" }), chatReady: false, signedIn: null }),
      provider("cursor", { runtime: runtime({ phase: "updateAvailable" }), signedIn: false }),
    ].map((p) => mount(ProviderRow, { props: { row: deriveRow(p) } }));

    const copy = rows.map((w) => w.text()).join(" ");
    expect(copy).toContain("Download failed");
    expect(copy).not.toMatch(/\b(agent|cli|acp|sdk|npx|path|runtime)\b/i);
  });
});
