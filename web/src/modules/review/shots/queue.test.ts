// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { newShotId, resetShotQueuesForTest, useShotQueue, type ShotItem } from "./queue";

const item = (note: string): ShotItem => ({
  id: newShotId(),
  side: "change",
  path: "/pricing",
  viewport: "desktop",
  rect: { x: 0, y: 0, width: 10, height: 10 },
  imageDataUrl: "data:image/png;base64,AA==",
  note,
  createdAt: "2026-11-19T12:00:00Z",
});

beforeEach(() => resetShotQueuesForTest());

describe("useShotQueue", () => {
  it("is shared by every view of the same change, and separate per change", async () => {
    const a = useShotQueue(() => "site", () => 7);
    a.add(item("Hero cut off"));
    const b = useShotQueue(() => "site", () => 7);
    await Promise.resolve();
    expect(b.items.value.map((i) => i.note)).toEqual(["Hero cut off"]);
    expect(useShotQueue(() => "site", () => 8).items.value).toEqual([]);
  });

  it("edits a note, removes an item and clears", () => {
    const q = useShotQueue(() => "site", () => 7);
    const first = item("one");
    q.add(first);
    q.add(item("two"));
    q.update(first.id, { note: "one, edited" });
    expect(q.items.value.map((i) => i.note)).toEqual(["one, edited", "two"]);
    q.remove(first.id);
    expect(q.items.value.map((i) => i.note)).toEqual(["two"]);
    q.clear();
    expect(q.items.value).toEqual([]);
  });
});
