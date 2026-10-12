// @vitest-environment happy-dom
import { beforeEach, expect, it } from "vitest";
import type { FileTree } from "@pierre/trees";
import { savedExpanded, trackExpanded } from "./treeExpansion";

type Row = { path: string; kind: "directory" | "file"; isExpanded: boolean };

function fakeTree() {
  let rows: Row[] = [];
  let search = "";
  let listener = () => {};
  const tree = {
    subscribe: (fn: () => void) => { listener = fn; return () => { listener = () => {}; }; },
    getSearchValue: () => search,
    getVisibleCount: () => rows.length,
    getVisibleRows: (start: number, end: number) => rows.slice(start, end),
  } as unknown as FileTree;
  return {
    tree,
    show(next: Row[], query = "") { rows = next; search = query; listener(); },
  };
}
const dir = (path: string, isExpanded: boolean): Row => ({ path, kind: "directory", isExpanded });

beforeEach(() => localStorage.clear());

it("has nothing saved until a tree is tracked", () => {
  expect(savedExpanded("files:p")).toBeNull();
});

it("saves open folders and keeps those hidden under a closed parent", () => {
  const { tree, show } = fakeTree();
  trackExpanded(tree, "files:p");
  show([dir("src/", true), dir("src/pages/", true), { path: "src/a.ts", kind: "file", isExpanded: false }]);
  expect(savedExpanded("files:p")).toEqual(["src/", "src/pages/"]);

  show([dir("src/", false)]);
  expect(savedExpanded("files:p")).toEqual(["src/pages/"]);
  expect(savedExpanded("files:other")).toBeNull();
});

it("ignores folders opened by a search", () => {
  const { tree, show } = fakeTree();
  trackExpanded(tree, "files:p");
  show([dir("src/", true), dir("lib/", false)]);
  show([dir("src/", false), dir("lib/", true)], "index");
  expect(savedExpanded("files:p")).toEqual(["src/"]);
});
