// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { installScrollbarHiding, SCROLLBAR_VISIBLE_MS } from "./scrollbars";

let stop: (() => void) | null = null;
beforeEach(() => { vi.useFakeTimers(); });
afterEach(() => { stop?.(); stop = null; vi.useRealTimers(); document.body.innerHTML = ""; });

it("reveals a scroller's scrollbar only while it moves", () => {
  stop = installScrollbarHiding();
  const list = document.createElement("ul");
  document.body.append(list);
  expect(list.hasAttribute("data-scrolling")).toBe(false);
  list.dispatchEvent(new Event("scroll"));
  expect(list.hasAttribute("data-scrolling")).toBe(true);
  vi.advanceTimersByTime(SCROLLBAR_VISIBLE_MS - 1);
  expect(list.hasAttribute("data-scrolling")).toBe(true);
  vi.advanceTimersByTime(1);
  expect(list.hasAttribute("data-scrolling")).toBe(false);
});

it("keeps the mark while scrolling and moves it to the next scroller", () => {
  stop = installScrollbarHiding();
  const a = document.createElement("div");
  const b = document.createElement("div");
  document.body.append(a, b);
  a.dispatchEvent(new Event("scroll"));
  vi.advanceTimersByTime(SCROLLBAR_VISIBLE_MS - 1);
  a.dispatchEvent(new Event("scroll"));
  vi.advanceTimersByTime(SCROLLBAR_VISIBLE_MS - 1);
  expect(a.hasAttribute("data-scrolling")).toBe(true);
  b.dispatchEvent(new Event("scroll"));
  expect(a.hasAttribute("data-scrolling")).toBe(false);
  expect(b.hasAttribute("data-scrolling")).toBe(true);
});

it("clears the mark when uninstalled", () => {
  stop = installScrollbarHiding();
  const list = document.createElement("ul");
  document.body.append(list);
  list.dispatchEvent(new Event("scroll"));
  stop();
  stop = null;
  expect(list.hasAttribute("data-scrolling")).toBe(false);
});
