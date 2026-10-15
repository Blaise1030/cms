import { beforeEach, describe, expect, it, vi } from "vitest";
import { toast } from "vue-sonner";
import type { Change, ChangesUpdate } from "@/core/types";

vi.mock("vue-sonner", () => ({ toast: { info: vi.fn(), success: vi.fn(), warning: vi.fn(), error: vi.fn() } }));

import { report } from "./getLatest";

const change = (branch: string, title: string) => ({ branch, title }) as Change;

function result(over: Partial<ChangesUpdate> = {}): ChangesUpdate {
  return { mainUpdated: false, mainDiverged: false, updated: [], conflicts: [], ...over };
}

/** Every string handed to any toast call. */
function toastedStrings(): string[] {
  const calls = [toast.info, toast.success, toast.warning, toast.error].flatMap((fn) => vi.mocked(fn).mock.calls);
  return calls.flatMap(([title, opts]) => [String(title), String((opts as { description?: string } | undefined)?.description ?? "")]);
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("report", () => {
  it("says the change is up to date with the live site", () => {
    report(result({ mainUpdated: true, updated: [change("content/a", "A")] }));
    expect(toast.success).toHaveBeenCalledWith("Up to date with the live site", expect.anything());
  });

  it("says it is syncing with the live site while the agent works", () => {
    report(result({ skipped: change("content/a", "A") }));
    expect(toast.info).toHaveBeenCalledWith("Syncing with the live site", expect.anything());
  });

  it("says the draft updated but the live site was left as is", () => {
    report(result({ mainDiverged: true, mainUpdated: true, updated: [change("content/a", "A")] }));
    expect(toast.warning).toHaveBeenCalledWith("Your draft is updated; the live site was left as is.", expect.anything());
  });

  it("stays quiet when the sync dialog takes over", () => {
    const a = change("content/a", "A");
    report(result({ conflicts: [a], conflictSessions: { "content/a": "s1" } }));
    expect(toastedStrings()).toEqual([]);
  });

  it("report_copy_is_plain", () => {
    const a = change("content/a", "A");
    report(result({ mainUpdated: true, updated: [a] }));
    report(result({ updated: [] }));
    report(result({ skipped: a }));
    report(result({ mainDiverged: true, mainUpdated: true, updated: [a], skipped: a }));
    const strings = toastedStrings();
    expect(strings.length).toBeGreaterThan(0);
    for (const s of strings) expect(s).not.toMatch(/main|merge|conflict|branch/i);
  });
});
