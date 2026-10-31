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
vi.mock("vue-sonner", () => ({ toast: { error: vi.fn(), success: vi.fn(), warning: vi.fn() } }));
import { toast } from "vue-sonner";
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

it("cannot start an entry whose required unsupported field has no default", () => {
  expect(startingFields(hero.fields as FieldDescriptor, "T")).toEqual({ reason: expect.stringContaining("image") });
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

it("creates an Untitled entry and opens its editor", async () => {
  const createContentEntry = vi.fn(async () => untitled);
  const w = await setup({ createContentEntry });
  await choose("blog");
  el("content-new-entry")!.click(); await flushPromises();
  expect(createContentEntry).toHaveBeenCalledWith({
    project: "p", collection: "blog", slug: "untitled", branch: "content/x",
    fields: { title: "Untitled", description: "", pubDate: today(), tags: [] }, body: "",
  });
  const store = useContentStore();
  expect(store.entry?.id).toBe("untitled");
  expect(store.creation).toBeNull();
  expect(el("content-create-dialog")).toBeNull();
  expect(el("content-entry-title")!.textContent).toBe("Untitled");
  expect(w.vm.$route.path).toBe("/p/p/content/blog/untitled");
});

it("walks past the untitled slugs the collection already has", async () => {
  const createContentEntry = vi.fn(async () => ({ ...untitled, id: "untitled-3" }));
  await setup({
    createContentEntry,
    listContentEntries: async () => [{ ...saved, id: "untitled" }, { ...saved, id: "untitled-2" }],
  });
  await choose("blog");
  el("content-new-entry")!.click(); await flushPromises();
  expect(createContentEntry).toHaveBeenCalledWith(expect.objectContaining({ slug: "untitled-3" }));
});

it("retries once when the server still reports a duplicate slug", async () => {
  const createContentEntry = vi.fn()
    .mockRejectedValueOnce(new ApiError("CONFLICT", "already exists", 409, { reason: "duplicate_slug" }))
    .mockResolvedValueOnce(untitled);
  await setup({ createContentEntry });
  await choose("blog");
  el("content-new-entry")!.click(); await flushPromises();
  expect(createContentEntry).toHaveBeenNthCalledWith(1, expect.objectContaining({ slug: "untitled" }));
  expect(createContentEntry).toHaveBeenNthCalledWith(2, expect.objectContaining({ slug: "untitled-2" }));
  expect(useContentStore().entry?.id).toBe("untitled");
});

it("names the fields the collection rejected when a create fails validation", async () => {
  const createContentEntry = vi.fn().mockRejectedValue(new ApiError("BADREQ", "Content does not match the collection schema", 422, {
    reason: "invalid_schema",
    details: [{ path: ["description"], message: "Required" }, { path: ["seo", "note"], message: "Too short" }],
  }));
  await setup({ createContentEntry });
  await choose("blog");
  el("content-new-entry")!.click(); await flushPromises();
  expect(vi.mocked(toast.error)).toHaveBeenLastCalledWith("Content does not match the collection schema", {
    description: "Description: Required\nSeo › Note: Too short",
  });
});

it("records why an entry could not be created", async () => {
  const createContentEntry = vi.fn().mockRejectedValue(new ApiError("BADREQ", "invalid slug", 400, { reason: "invalid_slug" }));
  await setup({ createContentEntry });
  await choose("blog");
  el("content-new-entry")!.click(); await flushPromises();
  const store = useContentStore();
  expect(store.entry).toBeNull();
  expect(store.needsChange).toBe(false);
  expect(store.creation?.error).toContain("invalid slug");
});

it("offers no Create for the singleton and explains a blocked collection", async () => {
  await setup();
  await choose("site");
  expect(el("content-new-entry")).toBeNull();
  el("content-back")!.click(); await flushPromises();
  await choose("heroes");
  expect(el<HTMLButtonElement>("content-new-entry")!.disabled).toBe(true);
  expect(el<HTMLButtonElement>("content-new-entry")!.title).toContain("image");
});

it("on main offers Create Change and resumes the create on the new branch", async () => {
  const createContentEntry = vi.fn()
    .mockRejectedValueOnce(new ApiError("CHANGE_REQUIRED", "Start a change first", 409))
    .mockResolvedValueOnce(untitled);
  await setup({ createContentEntry }, [blog], null);
  await choose("blog");
  el("content-new-entry")!.click(); await flushPromises();
  expect(createContentEntry).toHaveBeenLastCalledWith(expect.objectContaining({ branch: "main", slug: "untitled" }));
  expect(el("content-needs-change")).not.toBeNull();
  await useContentStore().resumeSave("content/spring-launch");
  await flushPromises();
  expect(createContentEntry).toHaveBeenLastCalledWith(expect.objectContaining({ branch: "content/spring-launch", slug: "untitled" }));
  expect(useContentStore().entry?.id).toBe("untitled");
  expect(el("content-needs-change")).toBeNull();
});
