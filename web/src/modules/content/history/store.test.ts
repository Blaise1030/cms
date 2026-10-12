import { expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { ApiError } from "@/core/api";
import type { ContentVersion, FileVersion } from "@/core/types";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
import { useContentHistoryStore } from "./store";

const versions: FileVersion[] = [
  { sha: "b2", summary: "edit", author: "Ann", ts: 2, path: "src/content/blog/post.md", published: false },
  { sha: "a1", summary: "create", author: "Ann", ts: 1, path: "src/content/blog/post.md", published: true },
];
const old: ContentVersion = {
  sha: "a1", path: "src/content/blog/post.md", source: "old", fields: { title: "Old" }, body: "old", parseable: true,
};

function setup(api: Record<string, unknown> = {}) {
  const contentHistory = vi.fn(async () => versions);
  const contentVersion = vi.fn(async () => old);
  h.daemon = createFakeDaemon({ contentHistory, contentVersion, ...api });
  setActivePinia(createPinia());
  return { store: useContentHistoryStore(), contentHistory, contentVersion };
}

it("open() loads versions for the entry", async () => {
  const { store, contentHistory } = setup();
  await store.open("p", "blog", "post");
  expect(store.key).toBe("blog/post");
  expect(store.versions.length).toBe(2);
  expect(contentHistory).toHaveBeenCalledWith("p", "blog", "post");
});

it("select() fetches the version", async () => {
  const { store, contentVersion } = setup();
  await store.open("p", "blog", "post");
  await store.select("a1");
  expect(contentVersion).toHaveBeenCalledWith("p", "blog", "post", "a1");
  expect(store.selectedSha).toBe("a1");
  expect(store.version?.source).toBe("old");
});

it("checkpoint.created reloads", async () => {
  const { store, contentHistory } = setup();
  await store.open("p", "blog", "post");
  h.daemon.emit({ type: "checkpoint.created", projectId: "p", sha: "c3", summary: "s", author: "Ann", ts: 3 });
  await flushPromises();
  expect(contentHistory).toHaveBeenCalledTimes(2);
});

it("select() 404 reloads and clears selection", async () => {
  const { store, contentHistory } = setup({
    contentVersion: vi.fn(async () => { throw new ApiError("UNKNOWN", "not found", 404); }),
  });
  await store.open("p", "blog", "post");
  await store.select("gone");
  await flushPromises();
  expect(store.selectedSha).toBeNull();
  expect(contentHistory).toHaveBeenCalledTimes(2);
  expect(store.error).toBeNull();
});

it("open() of another entry clears selection", async () => {
  const { store } = setup();
  await store.open("p", "blog", "post");
  await store.select("a1");
  await store.open("p", "blog", "other");
  expect(store.selectedSha).toBeNull();
  expect(store.version).toBeNull();
});
