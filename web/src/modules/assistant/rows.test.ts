import { describe, expect, it } from "vitest";
import type { RuntimeStatus, SetupProvider } from "@/core/types";
import {
  buildRows,
  clampProgress,
  deriveRow,
  LABELS,
  orderProviders,
  PROVIDER_DESCRIPTIONS,
  PROVIDER_NAMES,
  siteBuilderRow,
} from "./rows";

const runtime = (over: Partial<RuntimeStatus> = {}): RuntimeStatus => ({
  phase: "ready",
  version: "1.0.0",
  source: "managed",
  progress: null,
  message: null,
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

describe("row ordering", () => {
  it("puts the recommendation first, then the plan assistants", () => {
    const ordered = orderProviders([
      provider("cursor"),
      provider("claude"),
      provider("codex"),
      provider("opencode"),
    ]);
    expect(ordered.map((p) => p.id)).toEqual(["opencode", "codex", "claude", "cursor"]);
  });

  it("keeps unknown providers after the known ones in daemon order", () => {
    const ordered = orderProviders([
      provider("custom-b"),
      provider("opencode"),
      provider("custom-a"),
    ]);
    expect(ordered.map((p) => p.id)).toEqual(["opencode", "custom-b", "custom-a"]);
  });

  it("does not reorder rows when one changes state", () => {
    const before = buildRows([
      provider("cursor"),
      provider("opencode"),
      provider("claude"),
      provider("codex"),
    ]);
    const after = buildRows([
      provider("cursor", { runtime: runtime({ phase: "downloadError" }) }),
      provider("opencode", { runtime: runtime({ phase: "downloading", progress: 40 }) }),
      provider("claude", { runtime: runtime({ phase: "finishing" }) }),
      provider("codex", { signedIn: false }),
    ]);
    expect(after.map((r) => r.id)).toEqual(before.map((r) => r.id));
    expect(after.map((r) => r.id)).toEqual(["opencode", "codex", "claude", "cursor"]);
  });
});

describe("row state", () => {
  it.each([
    [
      "not downloaded",
      provider("opencode", {
        runtime: runtime({ phase: "notDownloaded", version: null, source: null }),
        installed: false,
        chatReady: false,
      }),
      { status: "", action: "download", actionLabel: LABELS.download, tone: "muted" },
    ],
    [
      "downloading",
      provider("opencode", { runtime: runtime({ phase: "downloading", progress: 42 }), chatReady: false }),
      { status: "42%", action: "cancel", actionLabel: LABELS.cancel, tone: "muted" },
    ],
    [
      "finishing",
      provider("opencode", { runtime: runtime({ phase: "finishing" }), chatReady: false }),
      { status: LABELS.settingUp, action: "none", actionLabel: "", tone: "muted" },
    ],
    [
      "download failed",
      provider("opencode", {
        runtime: runtime({ phase: "downloadError", message: "offline" }),
        chatReady: false,
      }),
      { status: LABELS.downloadFailed, action: "retry", actionLabel: LABELS.retry, tone: "destructive" },
    ],
    [
      "update available",
      provider("claude", { runtime: runtime({ phase: "updateAvailable" }) }),
      { status: LABELS.updateAvailable, action: "update", actionLabel: LABELS.update, tone: "warning" },
    ],
    [
      "ready and signed in",
      provider("claude"),
      { status: LABELS.connected, action: "connect", actionLabel: LABELS.connect, tone: "success" },
    ],
    [
      "ready and signed out",
      provider("claude", { signedIn: false, chatReady: false }),
      {
        status: LABELS.signInRequired,
        action: "signIn",
        actionLabel: LABELS.signIn,
        tone: "warning",
      },
    ],
    [
      "OpenCode with free models and no sign-in",
      provider("opencode", { signedIn: null, chatReady: true, freeModels: true }),
      { status: LABELS.connected, action: "signIn", actionLabel: LABELS.signIn, tone: "success" },
    ],
    [
      "OpenCode signed in",
      provider("opencode", { signedIn: true, chatReady: true, freeModels: true }),
      { status: LABELS.connected, action: "connect", actionLabel: LABELS.connect, tone: "success" },
    ],
    [
      "runtime ready but chat not",
      provider("codex", { signedIn: null, chatReady: false }),
      { status: LABELS.signInRequired, action: "signIn", actionLabel: LABELS.signIn, tone: "warning" },
    ],
  ])("derives %s", (_name, p, expected) => {
    expect(deriveRow(p)).toMatchObject(expected);
  });

  it("shows progress without a daemon percentage as Downloading", () => {
    const row = deriveRow(
      provider("opencode", { runtime: runtime({ phase: "downloading", progress: null }) }),
    );
    expect(row.status).toBe(LABELS.downloading);
    expect(row.action).toBe("cancel");
  });

  it("clamps progress into 0–100", () => {
    expect(clampProgress(140)).toBe(100);
    expect(clampProgress(-3)).toBe(0);
    expect(clampProgress(42.6)).toBe(43);
    expect(clampProgress(null)).toBeNull();
    expect(clampProgress(Number.NaN)).toBeNull();
  });

  it("falls back to a system copy for older daemons without a runtime", () => {
    const row = deriveRow({
      id: "claude",
      name: "Claude",
      available: true,
      installed: true,
      signedIn: true,
      chatReady: true,
    });
    expect(row).toMatchObject({ phase: "ready", source: "system", status: LABELS.connected });
  });
});

describe("site builder row", () => {
  it("uses the site builder copy and offers a download", () => {
    const row = siteBuilderRow(runtime({ phase: "notDownloaded", version: null, source: null }));
    expect(row).toMatchObject({
      id: "node",
      name: "Site builder",
      description: "Lets orso build and preview your site.",
      action: "download",
      actionLabel: LABELS.download,
      chatReady: false,
    });
  });

  it("is connected with nothing to press once ready", () => {
    const row = siteBuilderRow(runtime({ version: "24.0.0" }));
    expect(row).toMatchObject({
      status: LABELS.connected,
      action: "none",
      version: "24.0.0",
      chatReady: true,
    });
  });

  it("shows progress while downloading", () => {
    const row = siteBuilderRow(runtime({ phase: "downloading", progress: 12 }));
    expect(row).toMatchObject({ status: "12%", actionLabel: LABELS.cancel });
  });
});

describe("copy", () => {
  it("never uses developer words", () => {
    const copy = [
      ...Object.values(LABELS),
      ...Object.values(PROVIDER_NAMES),
      ...Object.values(PROVIDER_DESCRIPTIONS),
    ].join(" ");
    expect(copy).not.toMatch(/\b(agent|cli|acp|sdk|npx|path|runtime)\b/i);
  });

  it("names the recommendation and its free models", () => {
    const rows = buildRows([
      provider("claude"),
      provider("opencode", { freeModels: true, signedIn: null }),
    ]);
    expect(rows[0]).toMatchObject({
      id: "opencode",
      recommended: true,
      description: "Free models, no account needed.",
    });
    expect(rows[1]!.recommended).toBe(false);
  });
});
