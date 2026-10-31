// @vitest-environment happy-dom
import { expect, it } from "vitest";
import { entryActions, filterEntries, sortEntries } from "./entryList";

const row = (id: string) => ({ collection: "blog", id, path: `src/content/blog/${id}.md`, kind: "markdown" as const });
const handlers = { open: () => {}, copyLink: () => {}, copyPath: () => {}, rename: () => {}, remove: () => {} };
const ids = (actions: ReturnType<typeof entryActions>) =>
  actions.flatMap(a => (a === "separator" ? [] : [a.id]));

it("filters by a case-insensitive slug substring", () => {
  const rows = [row("Welcome"), row("spring-launch"), row("about")];
  expect(filterEntries(rows, "SPRING").map(r => r.id)).toEqual(["spring-launch"]);
  expect(filterEntries(rows, "")).toHaveLength(3);
  expect(filterEntries(rows, "zzz")).toEqual([]);
});

it("sorts rows by slug and leaves the input alone", () => {
  const rows = [row("b"), row("a"), row("c")];
  expect(sortEntries(rows).map(r => r.id)).toEqual(["a", "b", "c"]);
  expect(rows.map(r => r.id)).toEqual(["b", "a", "c"]);
});

it("offers open, link, path, rename and delete for an editable entry", () => {
  expect(ids(entryActions({ singleton: false, path: "src/content/blog/welcome.md", dirty: false, handlers })))
    .toEqual(["open", "copy-link", "copy-path", "rename", "delete"]);
});

it("omits the file path when the list did not supply one", () => {
  expect(ids(entryActions({ singleton: false, path: undefined, dirty: false, handlers })))
    .toEqual(["open", "copy-link", "rename", "delete"]);
});

it("keeps a singleton to open and link", () => {
  expect(ids(entryActions({ singleton: true, path: "src/content/site.json", dirty: false, handlers })))
    .toEqual(["open", "copy-link", "copy-path"]);
});

it("disables rename and delete while a draft is dirty", () => {
  const actions = entryActions({ singleton: false, path: "src/content/blog/welcome.md", dirty: true, handlers })
    .filter(a => a !== "separator");
  const rename = actions.find(a => a.id === "rename")!;
  const remove = actions.find(a => a.id === "delete")!;
  expect(rename.disabled).toBe(true);
  expect(rename.hint).toBe("Save or discard your changes first");
  expect(remove.disabled).toBe(true);
  expect(remove.destructive).toBe(true);
});
