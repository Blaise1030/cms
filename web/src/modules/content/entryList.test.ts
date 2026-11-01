// @vitest-environment happy-dom
import { expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";
import type { ContentCollection, ContentEntrySummary } from "@/core/types";
import type { EntryAction } from "./entryList";
import EntryList from "./EntryList.vue";

const blog: ContentCollection = { name: "blog", kind: "markdown", loader: {}, editable: true, reason: null };
const row = (id: string): ContentEntrySummary => ({ collection: "blog", id, path: `src/content/blog/${id}.md`, kind: "markdown" });
const rename = vi.fn();
const menu: EntryAction[] = [
  { id: "open", label: "Open", run: () => {} },
  "separator",
  { id: "rename", label: "Rename…", run: rename },
];

/** Render the menu inline (no portal, always open); items emit `select` on click. */
const stubs = {
  DropdownMenu: { template: "<div><slot /></div>" },
  DropdownMenuTrigger: { template: "<div><slot /></div>" },
  DropdownMenuContent: { template: "<div><slot /></div>" },
  DropdownMenuSeparator: { template: "<hr />" },
  DropdownMenuItem: {
    props: ["disabled", "asChild"],
    emits: ["select"],
    template: `<div v-if="!asChild" :data-disabled="disabled || undefined" @click="$emit('select')"><slot /></div>`,
  },
};

function mountList(over: Partial<{ collection: ContentCollection; entries: ContentEntrySummary[]; loading: boolean; createHint: string | null }> = {}) {
  return mount(EntryList, {
    props: { collection: blog, entries: [row("welcome"), row("spring-launch")], loading: false, createHint: null, actions: () => menu, ...over },
    global: { stubs },
  });
}

it("lists every entry with its slug as an addressable row", () => {
  const w = mountList();
  const rows = w.findAll('[data-testid="content-entry"]');
  expect(rows.map(r => r.attributes("data-entry-id"))).toEqual(["spring-launch", "welcome"]);
});

it("puts a search icon in the filter and matches the visible names", () => {
  const w = mountList();
  const group = w.get('[data-slot="input-group"]');
  expect(group.find("svg.lucide-search").exists()).toBe(true);
  expect(w.get('[data-testid="content-filter"]').attributes("placeholder")).toBe("Filter entries");
});

it("shows the collection name and the number of rows on screen", async () => {
  const w = mountList();
  expect(w.text()).toContain("Blog");
  expect(w.text()).toContain("2 entries");
  await w.get('[data-testid="content-filter"]').setValue("spring");
  expect(w.text()).toContain("1 entry");
});

it("filters by slug and says when nothing matches", async () => {
  const w = mountList();
  await w.get('[data-testid="content-filter"]').setValue("zzz");
  expect(w.findAll('[data-testid="content-entry"]')).toHaveLength(0);
  expect(w.text()).toContain("No matches");
});

it("says there are no entries yet and offers create", () => {
  const w = mountList({ entries: [] });
  expect(w.text()).toContain("No entries yet");
  expect(w.get('[data-testid="content-new-entry"]').attributes("disabled")).toBeUndefined();
});

it("disables create and carries the reason", () => {
  const w = mountList({ entries: [], createHint: "Add an image field first" });
  const button = w.get('[data-testid="content-new-entry"]');
  expect(button.attributes("disabled")).toBeDefined();
  expect(button.attributes("title")).toContain("Add an image field first");
});

it("opens an entry when its row is clicked", async () => {
  const w = mountList();
  await w.get('[data-entry-id="welcome"]').trigger("click");
  expect(w.emitted("open")).toEqual([["welcome"]]);
});

it("runs a row menu action", async () => {
  rename.mockClear();
  const w = mountList();
  await w.get('[data-testid="content-entry-rename"]').trigger("click");
  expect(rename).toHaveBeenCalledOnce();
});

it("always shows the row menu and hides the kind label", () => {
  const w = mountList();
  expect(w.text()).not.toContain("markdown");
  expect(w.get('[data-testid="content-entry-menu"]').classes()).not.toContain("opacity-0");
});

it("keeps the filter when the same collection reloads but clears it on a new collection", async () => {
  const w = mountList();
  await w.get('[data-testid="content-filter"]').setValue("spring");
  await w.setProps({ entries: [row("welcome")] });
  expect((w.get('[data-testid="content-filter"]').element as HTMLInputElement).value).toBe("spring");
  await w.setProps({ collection: { ...blog, name: "notes" } });
  expect((w.get('[data-testid="content-filter"]').element as HTMLInputElement).value).toBe("");
});
