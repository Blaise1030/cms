import { describe, expect, it } from "vitest";
import type { FileVersion } from "@/core/types";
import { versionBadge, versionDiff, versionTime } from "./history";

const version = (published: boolean): FileVersion => ({
  sha: "abc", summary: "Edit", author: "Ann", ts: 0, path: "a.md", published,
});

describe("versionBadge", () => {
  it("marks the newest version Current, then Published or Draft", () => {
    expect(versionBadge(0, version(true))).toBe("Current");
    expect(versionBadge(1, version(true))).toBe("Published");
    expect(versionBadge(1, version(false))).toBe("Draft");
  });
});

describe("versionTime", () => {
  const now = new Date(2026, 9, 13, 12);
  const ts = (d: Date) => d.getTime() / 1000;

  it("labels today, yesterday and older days", () => {
    expect(versionTime(ts(new Date(2026, 9, 13, 18, 36)), now)).toBe("Today at 6:36 PM");
    expect(versionTime(ts(new Date(2026, 9, 12, 18, 36)), now)).toBe("Yesterday at 6:36 PM");
    expect(versionTime(ts(new Date(2026, 9, 3, 18, 36)), now)).toBe("Oct 3 at 6:36 PM");
  });
});

describe("versionDiff", () => {
  it("diffs two sources of the same file", () => {
    expect(versionDiff("a.md", "x\n", "y\n").hunks.length).toBe(1);
  });
});
