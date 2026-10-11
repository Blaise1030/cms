// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from "vitest";
import { createRouter, createMemoryHistory, RouterView } from "vue-router";
import { mount, flushPromises, type VueWrapper } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import { ApiError } from "@/core/api";
import type { FieldDescriptor } from "@/core/types";
const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
import ContentPanel from "./ContentPanel.vue";
import { useContentStore } from "./store";
import { useChangesStore } from "@/modules/changes/store";
import { slugify, startingFields, today } from "./create";

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

it("slugifies titles", () => {
  expect(slugify("  Spring Launch: Notes & Ideas! ")).toBe("spring-launch-notes-ideas");
  expect(slugify("Café déjà vu")).toBe("cafe-deja-vu");
  expect(slugify("---")).toBe("");
});

it("starts entries from defaults, the title, today, and empty required values", () => {
  expect(startingFields(blogFields, "Hello", "2026-10-11")).toEqual({ fields: { title: "Hello", description: "", pubDate: "2026-10-11", tags: [] } });
  expect(today(new Date(2026, 0, 5))).toBe("2026-01-05");
  const section: FieldDescriptor = { type: "union", required: true, discriminator: "type", variants: [
    { type: "object", required: true, properties: { type: { type: "literal", required: true, values: ["hero"] }, heading: { type: "string", required: true }, count: { type: "number", required: true }, on: { type: "boolean", required: false } } },
  ] };
  expect(startingFields(section, "T", "2026-10-11")).toEqual({ fields: { type: "hero", heading: "", count: 0 } });
});

it("cannot start an entry whose required unsupported field has no default", () => {
  expect(startingFields(hero.fields as FieldDescriptor, "T")).toEqual({ reason: expect.stringContaining("image") });
  expect(startingFields({ type: "object", required: true, properties: { image: { type: "image", required: true, default: "./a.png" } } }, "T"))
    .toEqual({ fields: { image: "./a.png" } });
});

let w: VueWrapper | null = null;
afterEach(() => { w?.unmount(); w = null; document.body.innerHTML = ""; });
const el = <T extends HTMLElement = HTMLElement>(id: string) => document.body.querySelector(`[data-testid="${id}"]`) as T | null;
async function type(id: string, value: string) {
  const input = el<HTMLInputElement>(id)!; input.value = value; input.dispatchEvent(new Event("input")); await flushPromises();
}
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
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: "/", component: ContentPanel, props: { projectId: "p" } }] });
  await router.push("/"); await router.isReady();
  w = mount(RouterView, { global: { plugins: [router] }, attachTo: document.body });
  await flushPromises();
  return w;
}
async function choose(name: string) {
  const button = [...document.body.querySelectorAll<HTMLElement>('[data-testid="content-collection"]')].find(b => b.textContent?.startsWith(name))!;
  button.click(); await flushPromises();
}
const create = () => h.daemon.api as unknown as { createContentEntry: ReturnType<typeof vi.fn>; listContentEntries: ReturnType<typeof vi.fn> };

it("creates an entry from a title and defaulted slug on the change, then opens it", async () => {
  await setup();
  await choose("blog");
  el("content-create")!.click(); await flushPromises();
  await type("content-create-title", "Spring launch!");
  expect(el<HTMLInputElement>("content-create-slug")!.value).toBe("spring-launch");
  el("content-create-confirm")!.click(); await flushPromises();
  expect(create().createContentEntry).toHaveBeenCalledWith({
    project: "p", collection: "blog", slug: "spring-launch", branch: "content/x",
    fields: { title: "Spring launch!", description: "", pubDate: today(), tags: [] }, body: "",
  });
  const store = useContentStore();
  expect(store.entry?.id).toBe("spring-launch");
  expect(store.creation).toBeNull();
  expect(create().listContentEntries).toHaveBeenLastCalledWith("p", "blog");
  expect(el("content-create-dialog")).toBeNull();
});

it("keeps an edited slug when the title changes", async () => {
  await setup();
  await choose("blog");
  el("content-create")!.click(); await flushPromises();
  await type("content-create-slug", "custom");
  await type("content-create-title", "Something else");
  expect(el<HTMLInputElement>("content-create-slug")!.value).toBe("custom");
});

it("offers no Create for the singleton and explains a blocked collection", async () => {
  await setup();
  await choose("site");
  expect(el("content-create")).toBeNull();
  await choose("heroes");
  expect(el<HTMLButtonElement>("content-create")!.disabled).toBe(true);
  expect(el("content-create-reason")!.textContent).toContain("image");
});

it("shows duplicate and invalid slug errors and field errors in the dialog", async () => {
  const createContentEntry = vi.fn()
    .mockRejectedValueOnce(new ApiError("CONFLICT", "blog/spring-launch already exists", 409, { reason: "duplicate_slug" }))
    .mockRejectedValueOnce(new ApiError("BADREQ", "invalid slug", 400, { reason: "invalid_slug" }))
    .mockRejectedValueOnce(new ApiError("BADREQ", "invalid entry", 400, { reason: "invalid_schema", details: [{ path: ["description"], message: "Required" }] }));
  await setup({ createContentEntry });
  await choose("blog");
  el("content-create")!.click(); await flushPromises();
  await type("content-create-title", "Spring launch");
  el("content-create-confirm")!.click(); await flushPromises();
  expect(el("content-create-error")!.textContent).toContain("already exists");
  el("content-create-confirm")!.click(); await flushPromises();
  expect(el("content-create-error")!.textContent).toContain("invalid slug");
  el("content-create-confirm")!.click(); await flushPromises();
  expect(el("content-create-field-errors")!.textContent).toContain("description: Required");
  expect(useContentStore().entry).toBeNull();
  expect(el<HTMLInputElement>("content-create-title")!.value).toBe("Spring launch");
});

it("on main offers Create Change and resumes the create on the new branch", async () => {
  const createContentEntry = vi.fn()
    .mockRejectedValueOnce(new ApiError("CHANGE_REQUIRED", "Start a change first", 409))
    .mockResolvedValueOnce(saved);
  await setup({ createContentEntry }, [blog], null);
  await choose("blog");
  el("content-create")!.click(); await flushPromises();
  await type("content-create-title", "Spring launch!");
  el("content-create-confirm")!.click(); await flushPromises();
  expect(createContentEntry).toHaveBeenLastCalledWith(expect.objectContaining({ branch: "main" }));
  expect(el("content-needs-change")).not.toBeNull();
  expect(el("content-create-dialog")).toBeNull();
  await useContentStore().resumeSave("content/spring-launch");
  await flushPromises();
  expect(createContentEntry).toHaveBeenLastCalledWith(expect.objectContaining({ branch: "content/spring-launch", slug: "spring-launch" }));
  expect(useContentStore().entry?.id).toBe("spring-launch");
  expect(el("content-needs-change")).toBeNull();
});
