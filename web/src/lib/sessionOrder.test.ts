// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, it } from "vitest";
import {
  SESSION_ORDER_KEY,
  applySessionOrder,
  moveSessionId,
  readSessionOrder,
  writeSessionOrder,
} from "./sessionOrder";

beforeEach(() => localStorage.clear());
afterEach(() => localStorage.clear());

it("returns an empty order when nothing is stored", () => {
  expect(readSessionOrder("p1")).toEqual([]);
});

it("remembers and restores a project's session order", () => {
  writeSessionOrder("p1", ["c2", "c1"]);
  expect(localStorage.getItem(SESSION_ORDER_KEY("p1"))).toBe(JSON.stringify(["c2", "c1"]));
  expect(readSessionOrder("p1")).toEqual(["c2", "c1"]);
});

it("applies a saved order and appends unknown ids", () => {
  expect(applySessionOrder(["a", "b", "c"], ["c", "a"])).toEqual(["c", "a", "b"]);
  expect(applySessionOrder(["a", "b"], ["x", "b"])).toEqual(["b", "a"]);
});

it("moves an id before another for drag-reorder", () => {
  expect(moveSessionId(["a", "b", "c"], "c", "a")).toEqual(["c", "a", "b"]);
  expect(moveSessionId(["a", "b", "c"], "a", "c")).toEqual(["b", "a", "c"]);
  expect(moveSessionId(["a", "b", "c"], "b", "b")).toEqual(["a", "b", "c"]);
});
