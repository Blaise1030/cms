import { expect, it } from "vitest";
import type { ContentCollection, ContentEntrySummary } from "@/core/types";
import { treePaths, treeTarget } from "./contentTree";

const collection = (name: string) => ({ name, kind: "markdown", editable: true, reason: null, loader: {} }) as ContentCollection;
const entry = (c: string, id: string) => ({ collection: c, id, path: `src/content/${c}/${id}.md`, kind: "markdown" }) as ContentEntrySummary;

it("lists every collection as a folder, empty ones included, with its entries inside", () => {
  expect(treePaths([collection("blog"), collection("pages")], { blog: [entry("blog", "welcome"), entry("blog", "2024/launch")] }))
    .toEqual(["blog/", "blog/welcome", "blog/2024/launch", "pages/"]);
});

it("maps rows back to collections and entries", () => {
  expect(treeTarget("blog/", "directory")).toEqual({ collection: "blog" });
  expect(treeTarget("blog/welcome", "file")).toEqual({ collection: "blog", id: "welcome" });
  expect(treeTarget("blog/2024/launch", "file")).toEqual({ collection: "blog", id: "2024/launch" });
  expect(treeTarget("blog/2024/", "directory")).toBeNull();
});
