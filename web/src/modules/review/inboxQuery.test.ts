import { describe, expect, it } from "vitest";
import type { ReviewItem } from "@/core/types";
import { applyQuery, parseQuery, setQualifier } from "./inboxQuery";

const item = (over: Partial<ReviewItem>): ReviewItem => ({
  number: 1,
  title: "Fresh hero",
  author: "ada",
  url: "u",
  updatedAt: "2026-01-01T00:00:00Z",
  createdAt: "2026-01-01T00:00:00Z",
  headSha: "s",
  checks: "none",
  myState: "new",
  mine: false,
  labels: [{ name: "orso", color: "0e8a16" }],
  ...over,
});

describe("parseQuery", () => {
  it("reads GitHub-style qualifiers and keeps the rest as text", () => {
    expect(parseQuery('is:closed author:Bob label:"needs copy" review:sent_back sort:updated-desc Hero  #83')).toEqual({
      state: "closed",
      author: "Bob",
      label: "needs copy",
      review: "sent_back",
      sort: "updated",
      text: "hero #83",
    });
  });

  it("defaults to open, newest first, and treats unknown qualifiers as text", () => {
    expect(parseQuery("")).toEqual({ state: "open", author: null, label: null, review: null, sort: "newest", text: "" });
    expect(parseQuery("is:weird review:maybe foo:bar").text).toBe("is:weird review:maybe foo:bar");
    expect(parseQuery("sort:created-asc").sort).toBe("oldest");
  });
});

describe("setQualifier", () => {
  it("replaces, appends and removes a qualifier, keeping the rest", () => {
    expect(setQualifier("is:open hero author:ada", "author", "bob")).toBe("is:open hero author:bob");
    expect(setQualifier("is:open", "label", "needs copy")).toBe('is:open label:"needs copy"');
    expect(setQualifier('is:open label:"needs copy" hero', "label", null)).toBe("is:open hero");
    expect(setQualifier("is:open", "is", "closed")).toBe("is:closed");
  });
});

describe("applyQuery", () => {
  const items = [
    item({ number: 1, title: "Fresh hero", author: "ada", createdAt: "2026-01-02T00:00:00Z", updatedAt: "2026-01-09T00:00:00Z" }),
    item({ number: 2, title: "Pricing", author: "bob", myState: "greenlit", createdAt: "2026-01-03T00:00:00Z", labels: [{ name: "copy", color: "fff" }] }),
    item({ number: 3, title: "Blog post", author: "me", mine: true, createdAt: "2026-01-01T00:00:00Z" }),
    item({ number: 83, title: "Footer", author: "cy", myState: "sent_back", createdAt: "2026-01-04T00:00:00Z" }),
  ];
  const numbers = (q: string) => applyQuery(items, parseQuery(q)).map((i) => i.number);

  it("filters by text in the title, #number or author", () => {
    expect(numbers("hero")).toEqual([1]);
    expect(numbers("#83")).toEqual([83]);
    expect(numbers("BOB")).toEqual([2]);
    expect(numbers("nothing")).toEqual([]);
  });

  it("filters by author, label and review standing", () => {
    expect(numbers("author:ADA")).toEqual([1]);
    expect(numbers("label:copy")).toEqual([2]);
    expect(numbers("review:new")).toEqual([1]);
    expect(numbers("review:greenlit")).toEqual([2]);
    expect(numbers("review:sent_back")).toEqual([83]);
    expect(numbers("review:yours")).toEqual([3]);
  });

  it("sorts newest, oldest or recently updated", () => {
    expect(numbers("")).toEqual([83, 2, 1, 3]);
    expect(numbers("sort:created-asc")).toEqual([3, 1, 2, 83]);
    expect(numbers("sort:updated-desc")[0]).toBe(1);
  });
});
