// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from "vitest";
import { RouterView } from "vue-router";
import { contentRouter } from "@/test/contentRouter";
import { mount, flushPromises, type VueWrapper } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import { ApiError } from "@/core/api";
import type { FieldDescriptor } from "@/core/types";
const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
import { useContentStore } from "./store";
import { useChangesStore } from "@/modules/changes/store";
import { startingFields, today } from "./create";

const blogFields: FieldDescriptor = { type: "object", required: true, properties: {
  title: { type: "string", required: true },
  description: { type: "string", required: true },
  pubDate: { type: "date", required: true },
  coverImage: { type: "image", required: false },
  tags: { type: "array", required: false, default: [], items: { type: "string", required: true } },
} };
const blog = { name: "blog", kind: "markdown", editable: true, reason: null, loader: {}, fields: blogFields };
const site = { name: "site", kind: "json", singleton: "site", editable: true, reason: null, loader: {}, fields: { type: "object", required: true, properties: {} } };
const hero = { name: "heroes", kind: "markdown", editable: true, reason: null, loader: {}, fields: { type: "object", required: true, properties: {
  image: { type: "image", required: true },
} } };
const saved = { collection: "blog", id: "spring-launch", path: "src/content/blog/en/spring-launch.md", kind: "markdown", source: "S", fields: { title: "Spring launch!" }, body: "", branch: "content/x", hash: "h1" };

it("starts entries from defaults, the title, today, and empty required values", () => {
  expect(startingFields(blogFields, "Hello", "2026-10-11")).toEqual({ fields: { title: "Hello", description: "", pubDate: "2026-10-11", tags: [] } });
  expect(today(new Date(2026, 0, 5))).toBe("2026-01-05");
  const section: FieldDescriptor = { type: "union", required: true, discriminator: "type", variants: [
    { type: "object", required: true, properties: { type: { type: "literal", required: true, values: ["hero"] }, heading: { type: "string", required: true }, count: { type: "number", required: true }, on: { type: "boolean", required: false } } },
  ] };
  expect(startingFields(section, "T", "2026-10-11")).toEqual({ fields: { type: "hero", heading: "", count: 0 } });
});

it("fills a schema slug field with the entry's own slug", () => {
  const withSlug: FieldDescriptor = { type: "object", required: true, properties: {
    title: { type: "string", required: true },
    slug: { type: "string", required: true },
    note: { type: "string", required: false },
  } };
  expect(startingFields(withSlug, "Untitled", "2026-10-31", "untitled")).toEqual({ fields: { title: "Untitled", slug: "untitled" } });
  // A nested slug is not the document's address and stays empty.
  const nested: FieldDescriptor = { type: "object", required: true, properties: {
    seo: { type: "object", required: true, properties: { slug: { type: "string", required: true } } },
  } };
  expect(startingFields(nested, "T", "2026-10-31", "untitled")).toEqual({ fields: { seo: { slug: "" } } });
});

it("starts required references and images empty, and refuses an unsupported one", () => {
  expect(startingFields(hero.fields as FieldDescriptor, "T")).toEqual({ fields: { image: "" } });
  expect(startingFields({ type: "object", required: true, properties: { custom: { type: "unsupported", required: true, reason: "Unknown refinement" } } }, "T"))
    .toEqual({ reason: expect.stringContaining("custom") });
  expect(startingFields({ type: "object", required: true, properties: { image: { type: "image", required: true, default: "./a.png" } } }, "T"))
    .toEqual({ fields: { image: "./a.png" } });
});

let w: VueWrapper | null = null;
afterEach(() => { w?.unmount(); w = null; document.body.innerHTML = ""; });
const el = <T extends HTMLElement = HTMLElement>(id: string) => document.body.querySelector(`[data-testid="${id}"]`) as T | null;
async function setup(api: Record<string, unknown> = {}, collections: object[] = [blog, site, hero], branch: string | null = "content/x") {
  h.daemon = createFakeDaemon({
    listContentCollections: async () => collections,
    listContentEntries: vi.fn(async () => []),
    contentEntry: async () => saved,
    createContentEntry: vi.fn(async () => saved),
    ...api,
  });
  setActivePinia(createPinia());
  if (branch) useChangesStore().active = { branch, title: "X" };
  const router = await contentRouter();
  w = mount(RouterView, { global: { plugins: [router] }, attachTo: document.body });
  await flushPromises();
  return w;
}
async function choose(name: string) {
  const button = document.body.querySelector<HTMLElement>(`[data-testid="content-collection"][data-collection-name="${name}"]`)!;
  button.click(); await flushPromises();
}

const untitled = { ...saved, id: "untitled", path: "src/content/blog/en/untitled.md", fields: { title: "Untitled" } };

it("opens a new entry as an unsaved draft and creates it on save", async () => {
  const createContentEntry = vi.fn(async () => untitled);
  const w = await setup({ createContentEntry });
  await choose("blog");
  el("content-new-entry")!.click(); await flushPromises();
  // No file is written yet: the form opens with the schema's defaults.
  expect(createContentEntry).not.toHaveBeenCalled();
  expect(el<HTMLInputElement>("content-slug")!.value).toBe("untitled");
  expect(el<HTMLInputElement>("field-title")!.value).toBe("Untitled");
  expect(el("content-form")).not.toBeNull();
  const input = el<HTMLInputElement>("field-title")!;
  input.value = "Spring launch!";
  input.dispatchEvent(new Event("input")); await flushPromises();
  el("content-save")!.click(); await flushPromises();
  expect(createContentEntry).toHaveBeenCalledWith({
    project: "p", collection: "blog", slug: "untitled", branch: "content/x",
    fields: { title: "Spring launch!", description: "", pubDate: today(), tags: [] }, body: "",
  });
  expect(useContentStore().entry?.id).toBe("untitled");
  expect(useContentStore().creating).toBe(false);
  expect(w.vm.$route.path).toBe("/p/p/content/blog/untitled");
});

it("discards a draft from the back link without creating anything", async () => {
  const createContentEntry = vi.fn();
  await setup({ createContentEntry });
  await choose("blog");
  el("content-new-entry")!.click(); await flushPromises();
  el("content-back")!.click(); await flushPromises();
  expect(createContentEntry).not.toHaveBeenCalled();
  expect(useContentStore().creating).toBe(false);
  expect(el("content-entry-list")).not.toBeNull();
});

it("walks past the untitled slugs the collection already has", async () => {
  await setup({ listContentEntries: async () => [{ ...saved, id: "untitled" }, { ...saved, id: "untitled-2" }] });
  await choose("blog");
  el("content-new-entry")!.click(); await flushPromises();
  expect(el<HTMLInputElement>("content-slug")!.value).toBe("untitled-3");
});

it("leaves the draft open when the collection rejects the save", async () => {
  const createContentEntry = vi.fn().mockRejectedValue(new ApiError("BADREQ", "Content does not match the collection schema", 422, {
    reason: "invalid_schema",
    details: [{ path: ["description"], message: "Required" }],
  }));
  await setup({ createContentEntry });
  await choose("blog");
  el("content-new-entry")!.click(); await flushPromises();
  el("content-save")!.click(); await flushPromises();
  const store = useContentStore();
  expect(store.creating).toBe(true);
  expect(el("content-field-errors")!.textContent).toContain("description: Required");
  expect(el("content-form")).not.toBeNull();
});

it("creates a document whose required reference has no default", async () => {
  const help = { name: "helpDocs", kind: "markdown", editable: true, reason: null, fields: {
    type: "object", required: true, properties: {
      title: { type: "string", required: true },
      section: { type: "reference", required: true, collection: "helpCategories" },
    },
  } };
  const createContentEntry = vi.fn(async () => ({ ...untitled, collection: "helpDocs", id: "untitled" }));
  await setup({ createContentEntry, listContentCollections: async () => [help], listContentEntries: async () => [] });
  await choose("helpDocs");
  expect(el<HTMLButtonElement>("content-new-entry")!.disabled).toBe(false);
  el("content-new-entry")!.click(); await flushPromises();
  expect(el("content-form")).not.toBeNull();
  expect(el("field-section")).not.toBeNull();
});

it("offers no Create for the singleton, but opens a form for a required image", async () => {
  await setup();
  await choose("site");
  expect(el("content-new-entry")).toBeNull();
  el("content-back")!.click(); await flushPromises();
  await choose("heroes");
  expect(el<HTMLButtonElement>("content-new-entry")!.disabled).toBe(false);
  el("content-new-entry")!.click(); await flushPromises();
  expect(el("field-image-uploader")).not.toBeNull();
});

it("disables create when a required field cannot be edited at all", async () => {
  const custom = { name: "raw", kind: "markdown", editable: true, reason: null, fields: {
    type: "object", required: true, properties: {
      title: { type: "string", required: true },
      custom: { type: "unsupported", required: true, reason: "Unknown refinement" },
    },
  } };
  await setup({ listContentCollections: async () => [custom] });
  await choose("raw");
  expect(el<HTMLButtonElement>("content-new-entry")!.disabled).toBe(true);
  expect(el<HTMLButtonElement>("content-new-entry")!.title).toContain("custom");
});

it("on main offers Create Change when saving a draft", async () => {
  const createContentEntry = vi.fn()
    .mockRejectedValueOnce(new ApiError("CHANGE_REQUIRED", "Start a change first", 409))
    .mockResolvedValueOnce(untitled);
  await setup({ createContentEntry }, [blog], null);
  await choose("blog");
  el("content-new-entry")!.click(); await flushPromises();
  el("content-save")!.click(); await flushPromises();
  expect(createContentEntry).toHaveBeenLastCalledWith(expect.objectContaining({ branch: "main", slug: "untitled" }));
  expect(el("content-needs-change")).not.toBeNull();
  await useContentStore().resumeSave("content/spring-launch");
  await flushPromises();
  expect(createContentEntry).toHaveBeenLastCalledWith(expect.objectContaining({ branch: "content/spring-launch", slug: "untitled" }));
  expect(useContentStore().entry?.id).toBe("untitled");
  expect(el("content-needs-change")).toBeNull();
});
