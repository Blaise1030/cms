// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { defineComponent, h, nextTick, ref } from "vue";
import { mount } from "@vue/test-utils";
import { useReviewRefresh } from "./useReviewRefresh";

const refresh = vi.fn();
const source = ref("a");
const Host = defineComponent({
  setup() {
    useReviewRefresh(refresh, () => source.value);
    return () => h("div");
  },
});
let visibility = "visible";

beforeEach(() => {
  vi.useFakeTimers();
  refresh.mockClear();
  source.value = "a";
  visibility = "visible";
  vi.spyOn(document, "visibilityState", "get").mockImplementation(() => visibility as DocumentVisibilityState);
});
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

it("refreshes on mount and every 60 s while visible, skipping while hidden", () => {
  const w = mount(Host);
  expect(refresh).toHaveBeenCalledTimes(1);
  vi.advanceTimersByTime(60_000);
  expect(refresh).toHaveBeenCalledTimes(2);
  visibility = "hidden";
  vi.advanceTimersByTime(60_000);
  expect(refresh).toHaveBeenCalledTimes(2);
  w.unmount();
});

it("refreshes when the source changes", async () => {
  const w = mount(Host);
  refresh.mockClear();
  source.value = "b";
  await nextTick();
  expect(refresh).toHaveBeenCalledTimes(1);
  w.unmount();
});

it("clears its timer and listeners on unmount", () => {
  const w = mount(Host);
  w.unmount();
  refresh.mockClear();
  vi.advanceTimersByTime(180_000);
  window.dispatchEvent(new Event("focus"));
  document.dispatchEvent(new Event("visibilitychange"));
  expect(refresh).not.toHaveBeenCalled();
});

it("collapses a focus and visibilitychange pair into one refresh", () => {
  const w = mount(Host);
  vi.advanceTimersByTime(30_000);
  refresh.mockClear();
  window.dispatchEvent(new Event("focus"));
  document.dispatchEvent(new Event("visibilitychange"));
  expect(refresh).toHaveBeenCalledTimes(1);
  // Another wake inside 10 s is ignored; one after it refreshes again.
  vi.advanceTimersByTime(5_000);
  window.dispatchEvent(new Event("focus"));
  expect(refresh).toHaveBeenCalledTimes(1);
  vi.advanceTimersByTime(6_000);
  document.dispatchEvent(new Event("visibilitychange"));
  expect(refresh).toHaveBeenCalledTimes(2);
  w.unmount();
});

it("ignores a wake right after mount", () => {
  const w = mount(Host);
  window.dispatchEvent(new Event("focus"));
  document.dispatchEvent(new Event("visibilitychange"));
  expect(refresh).toHaveBeenCalledTimes(1);
  w.unmount();
});
