import { describe, expect, it } from "vitest";
import type { TimelineItem } from "@/core/types";
import { buildFeed, dayLabel, GROUP_MS } from "./feed";

const NOW = new Date(2026, 10, 19, 15, 0);
const at = (d: Date) => d.toISOString();
const comment = (id: number, author: string, when: Date): TimelineItem => ({
  kind: "comment", id, author, at: at(when), body: `c${id}`, url: `u${id}`,
});

describe("buildFeed", () => {
  it("keeps the timeline's oldest-first order, with a divider before each day", () => {
    const rows = buildFeed(
      [
        comment(1, "ada", new Date(2026, 10, 17, 9, 0)),
        comment(2, "bo", new Date(2026, 10, 18, 9, 0)),
        { kind: "state", at: at(new Date(2026, 10, 19, 10, 0)), state: "merged" },
      ],
      NOW,
    );
    expect(rows.map((r) => (r.type === "day" ? `day:${r.label}` : r.type))).toEqual([
      `day:${dayLabel(new Date(2026, 10, 17), NOW)}`,
      "message",
      "day:Yesterday",
      "message",
      "day:Today",
      "event",
    ]);
  });

  it("groups one author's messages within five minutes, and starts again after anything else", () => {
    const t = new Date(2026, 10, 19, 9, 0).getTime();
    const rows = buildFeed(
      [
        comment(1, "ada", new Date(t)),
        comment(2, "ada", new Date(t + 60_000)),
        comment(3, "ada", new Date(t + 60_000 + GROUP_MS + 1)),
        comment(4, "bo", new Date(t + 60_000 + GROUP_MS + 2)),
        { kind: "review", author: "bo", at: at(new Date(t + GROUP_MS * 2)), verdict: "greenlit", body: null },
        comment(5, "bo", new Date(t + GROUP_MS * 2 + 1)),
      ],
      NOW,
    ).filter((r) => r.type !== "day");
    expect(rows.map((r) => (r.type === "message" ? `${r.author}${r.grouped ? "+" : ""}` : r.type))).toEqual([
      "ada",
      "ada+",
      "ada",
      "bo",
      "event",
      "bo",
    ]);
  });

  it("treats a comment-only review as a message and a verdict as an event", () => {
    const rows = buildFeed(
      [
        { kind: "review", author: "cy", at: at(NOW), verdict: "commented", body: "Nit", url: "https://gh/r/1" },
        { kind: "review", author: "cy", at: at(NOW), verdict: "sent_back", body: "Fix it" },
      ],
      NOW,
    ).filter((r) => r.type !== "day");
    expect(rows[0]).toMatchObject({ type: "message", author: "cy", body: "Nit", url: "https://gh/r/1" });
    expect(rows[1]).toMatchObject({ type: "event", item: { verdict: "sent_back", body: "Fix it" } });
  });
});

describe("dayLabel", () => {
  it("says Today, Yesterday, else the date with the year only when it differs", () => {
    expect(dayLabel(new Date(2026, 10, 19, 1), NOW)).toBe("Today");
    expect(dayLabel(new Date(2026, 10, 18, 23), NOW)).toBe("Yesterday");
    expect(dayLabel(new Date(2026, 10, 10), NOW)).not.toMatch(/2026/);
    expect(dayLabel(new Date(2025, 10, 10), NOW)).toMatch(/2025/);
  });
});
