// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, it } from "vitest";
import {
  SESSION_ORDER_KEY,
  applySessionOrder,
  moveSessionId,
  moveSessionToIndex,
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

it("moves an id into another's place for drag-reorder, the last place included", () => {
  expect(moveSessionId(["a", "b", "c"], "c", "a")).toEqual(["c", "a", "b"]);
  expect(moveSessionId(["a", "b", "c"], "a", "c")).toEqual(["b", "c", "a"]);
  expect(moveSessionId(["a", "b", "c"], "a", "b")).toEqual(["b", "a", "c"]);
  expect(moveSessionId(["a", "b", "c"], "b", "b")).toEqual(["a", "b", "c"]);
  expect(moveSessionId(["a", "b", "c"], "a", "x")).toEqual(["a", "b", "c"]);
});

it("moves an id to an index, clamped to the ends", () => {
  expect(moveSessionToIndex(["a", "b", "c"], "a", 1)).toEqual(["b", "a", "c"]);
  expect(moveSessionToIndex(["a", "b", "c"], "c", -1)).toEqual(["c", "a", "b"]);
  expect(moveSessionToIndex(["a", "b", "c"], "a", 9)).toEqual(["b", "c", "a"]);
});
