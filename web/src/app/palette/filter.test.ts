import { expect, it } from "vitest";
import { filterGroups, SEARCH_LIMIT, type PaletteGroup } from "./filter";

const item = (label: string, hint?: string) => ({ id: label, label, hint, run: () => {} });

const groups: PaletteGroup[] = [
  { heading: "Navigate", items: [item("Go to Content"), item("Go to Code")] },
  { heading: "Files", searchOnly: true, items: Array.from({ length: 20 }, (_, i) => item(`file${i}.ts`, "src/lib")) },
];

it("shows every regular group but no search-only group before typing", () => {
  expect(filterGroups(groups, "  ").map((g) => g.heading)).toEqual(["Navigate"]);
});

it("matches all terms across label and hint, case-insensitively", () => {
  const out = filterGroups(groups, "LIB file1");
  expect(out.map((g) => g.heading)).toEqual(["Files"]);
  expect(out[0].items.every((i) => i.label.startsWith("file1"))).toBe(true);
});

it("caps search-only groups", () => {
  expect(filterGroups(groups, "file")[0].items).toHaveLength(SEARCH_LIMIT);
});

it("ranks label hits before hint-only hits", () => {
  const out = filterGroups([{ heading: "Files", items: [item("index.ts", "src/content"), item("content.md", "docs")] }], "content");
  expect(out[0].items.map((i) => i.label)).toEqual(["content.md", "index.ts"]);
});
