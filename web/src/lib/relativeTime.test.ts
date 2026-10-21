import { describe, expect, it } from "vitest";
import { relativeTime } from "./relativeTime";

const now = Date.parse("2026-10-21T12:00:00Z");

describe("relativeTime", () => {
  it("uses the largest whole unit", () => {
    expect(relativeTime("2026-10-21T11:59:00Z", now)).toBe("1 minute ago");
    expect(relativeTime("2026-10-21T09:00:00Z", now)).toBe("3 hours ago");
    expect(relativeTime("2026-10-20T12:00:00Z", now)).toBe("1 day ago");
    expect(relativeTime("2026-10-14T12:00:00Z", now)).toBe("1 week ago");
    expect(relativeTime("2026-09-21T12:00:00Z", now)).toBe("1 month ago");
    expect(relativeTime("2025-10-21T12:00:00Z", now)).toBe("1 year ago");
  });

  it("says 'just now' under a minute", () => {
    expect(relativeTime("2026-10-21T11:59:30Z", now)).toBe("just now");
  });

  it("keeps a positive direction for clock skew", () => {
    expect(relativeTime("2026-10-21T14:00:00Z", now)).toBe("in 2 hours");
  });

  it("returns null when there is no usable timestamp", () => {
    expect(relativeTime(null, now)).toBeNull();
    expect(relativeTime(undefined, now)).toBeNull();
    expect(relativeTime("", now)).toBeNull();
    expect(relativeTime("not a date", now)).toBeNull();
  });
});
