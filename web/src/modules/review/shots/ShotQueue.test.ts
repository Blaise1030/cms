// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DOMWrapper, flushPromises, mount } from "@vue/test-utils";
import { ApiError } from "@/core/api";
import { createFakeDaemon } from "@/test/fakeDaemon";
import { queryPlugin } from "@/test/queryClient";
import { newShotId, resetShotQueuesForTest, useShotQueue, type ShotItem } from "./queue";

const h = vi.hoisted(() => ({ daemon: null as unknown }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import ShotQueue from "./ShotQueue.vue";

const item = (note: string, image = true): ShotItem => ({
  id: newShotId(),
  side: "change",
  path: "/pricing",
  viewport: "desktop",
  rect: { x: 0, y: 0, width: 10, height: 10 },
  imageDataUrl: image ? "data:image/png;base64,iVBOR==" : null,
  note,
  createdAt: "2026-11-19T12:00:00Z",
});
const body = () => new DOMWrapper(document.body);

async function mountQueue(reviewWithShots: ReturnType<typeof vi.fn>) {
  h.daemon = createFakeDaemon({ reviewWithShots });
  const w = mount(ShotQueue, {
    props: { projectId: "site", number: 7, headSha: "abc", open: true },
    global: { plugins: [queryPlugin().plugin] },
    attachTo: document.body,
  });
  await flushPromises();
  await w.get("[data-testid='shot-queue-open']").trigger("click");
  await flushPromises();
  return w;
}

beforeEach(() => resetShotQueuesForTest());
afterEach(() => (document.body.innerHTML = ""));

describe("ShotQueue", () => {
  it("lists queued comments to edit and remove, and sends them as one comment review", async () => {
    const q = useShotQueue(() => "site", () => 7);
    q.add(item("Hero cut off"));
    q.add(item("Old copy", false));
    q.add(item("Remove me"));
    const reviewWithShots = vi.fn(async () => undefined);
    const w = await mountQueue(reviewWithShots);
    expect(w.get("[data-testid='shot-queue-open']").text()).toBe("Queued (3)");
    expect(body().findAll("[data-testid='shot-queue-item']")).toHaveLength(3);
    await body().findAll("[data-testid='shot-queue-note']")[0]!.setValue("Hero is cut off on mobile");
    await body().findAll("[data-testid='shot-queue-remove']")[2]!.trigger("click");
    expect(body().findAll("[data-testid='shot-queue-item']")).toHaveLength(2);

    await body().get("[data-testid='shot-queue-send']").trigger("click");
    await flushPromises();
    expect(reviewWithShots).toHaveBeenCalledWith("site", 7, {
      event: "COMMENT",
      body: "",
      headSha: "abc",
      items: [
        { note: "Hero is cut off on mobile", path: "/pricing", side: "change", viewport: "desktop", imageBase64: "iVBOR==", mime: "image/png" },
        { note: "Old copy", path: "/pricing", side: "change", viewport: "desktop" },
      ],
    });
    expect(q.items.value).toEqual([]);
  });

  it("keeps the queue and says why when sending fails", async () => {
    const q = useShotQueue(() => "site", () => 7);
    q.add(item("Hero cut off"));
    const reviewWithShots = vi.fn(async () => {
      throw new ApiError("UNKNOWN", "This change was updated since you opened it", 409, { reason: "head_moved" });
    });
    await mountQueue(reviewWithShots);
    await body().get("[data-testid='shot-queue-send']").trigger("click");
    await flushPromises();
    expect(body().get("[data-testid='shot-queue-error']").text()).toContain("updated since you opened it");
    expect(q.items.value).toHaveLength(1);
  });
});
