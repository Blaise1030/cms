import { describe, expect, it } from "vitest";
import { formatDownloadSize, sourceLabel, toneBadge } from "./presentation";

describe("formatDownloadSize", () => {
  it("rounds to a friendly approximate size", () => {
    expect(formatDownloadSize(157_286_400)).toBe("About 150 MB");
    expect(formatDownloadSize(10_066_329)).toBe("About 9.6 MB");
    expect(formatDownloadSize(900 * 1024)).toBe("About 900 KB");
    expect(formatDownloadSize(2 * 1024 * 1024 * 1024)).toBe("About 2 GB");
  });

  it("has nothing to say without a usable size", () => {
    expect(formatDownloadSize(null)).toBeNull();
    expect(formatDownloadSize(undefined)).toBeNull();
    expect(formatDownloadSize(0)).toBeNull();
    expect(formatDownloadSize(Number.NaN)).toBeNull();
  });
});

describe("sourceLabel", () => {
  it("describes where a ready copy came from without developer words", () => {
    expect(sourceLabel("managed")).toBe("Downloaded by orso");
    expect(sourceLabel("system")).toBe("Already on your Mac");
    expect(sourceLabel(null)).toBeNull();
  });
});

describe("toneBadge", () => {
  it("tints success and warning rows and marks errors destructive", () => {
    expect(toneBadge("success")).toMatchObject({ variant: "secondary" });
    expect(toneBadge("warning")).toMatchObject({ variant: "outline" });
    expect(toneBadge("destructive")).toMatchObject({ variant: "destructive" });
    expect(toneBadge("muted")).toBeNull();
  });
});
