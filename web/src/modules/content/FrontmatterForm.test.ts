// @vitest-environment happy-dom
import { expect, it, vi } from "vitest";
import { nextTick, reactive } from "vue";
import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import DatePicker from "./DatePicker.vue";
import USelect from "@nuxt/ui/components/Select.vue";
import UInputTags from "@nuxt/ui/components/InputTags.vue";
import FrontmatterForm from "./FrontmatterForm.vue";
import { REFERENCE_ENTRIES } from "./references";
import type { FieldDescriptor } from "@/core/types";
/** A Nuxt UI component wrapper; its generic props type does not survive `findComponent`. */
type AnyWrapper = Omit<VueWrapper, "props"> & { props(key: string): unknown };

const str: FieldDescriptor = { type: "string", required: true };
const descriptor: FieldDescriptor = {
  type: "object", required: true,
  properties: {
    title: str,
    nick: { type: "string", required: false },
    pubDate: { type: "date", required: true },
    kind: { type: "enum", required: true, values: ["news", "guide"] },
    featured: { type: "boolean", required: true },
    order: { type: "number", required: true },
    tags: { type: "array", required: false, items: str },
    seo: { type: "object", required: true, properties: { noindex: { type: "boolean", required: true } } },
    section: { type: "union", required: true, discriminator: "type", variants: [
      { type: "object", required: true, properties: { type: { type: "literal", required: true, values: ["hero"] }, heading: str } },
      { type: "object", required: true, properties: { type: { type: "literal", required: true, values: ["cta"] }, label: str } },
    ] },
    related: { type: "array", required: false, items: { type: "reference", required: true, collection: "blog" } },
    coverImage: { type: "image", required: false },
    custom: { type: "unsupported", required: true, reason: "Unknown custom refinement cannot be represented" },
  },
};
function render(value: Record<string, unknown>) {
  const w = mount(FrontmatterForm, { props: { descriptor, modelValue: value, errors: {}, "onUpdate:modelValue": (v: unknown) => w.setProps({ modelValue: v }) } });
  return w;
}
const value = () => ({ title: "Hi", pubDate: "2026-01-02", kind: "news", featured: true, order: 3, tags: ["a"], seo: { noindex: false }, section: { type: "hero", heading: "H" }, related: ["welcome"], coverImage: "src/assets/blog/x.png", custom: { a: 1 } });

it("calls the document's own title the display title", async () => {
  const w = render({});
  // UFormField points its label at the input's id once the input has mounted.
  await nextTick();
  expect(w.get('label[for="field-title"]').text()).toContain("Display title");
  // A title nested inside another object keeps its plain name.
  const nested = mount(FrontmatterForm, {
    props: { descriptor: { type: "object", required: true, properties: { title: str } }, label: "seo", path: ["seo"], modelValue: { title: "x" } },
  });
  await nextTick();
  expect(nested.get('label[for="field-seo.title"]').text()).toContain("Title");
  expect(nested.get('label[for="field-seo.title"]').text()).not.toContain("Display");
});

it("shows a placeholder named after the field in empty text inputs", () => {
  const w = render({});
  expect(w.get('[data-testid="field-nick"]').attributes("placeholder")).toBe("Enter nick");
  expect(w.get('[data-testid="field-order"]').attributes("placeholder")).toBe("Enter order");
  const camel = mount(FrontmatterForm, { props: { descriptor: str, label: "coverAlt", modelValue: "" } });
  expect(camel.get("input").attributes("placeholder")).toBe("Enter cover alt");
});

/** The Select whose trigger carries `id`; selects are driven through their model since their menus are portalled. */
function select(w: VueWrapper, id: string) {
  const found = w.findAllComponents(USelect).find(s => s.find(`[data-testid="${id}"]`).exists()) as unknown as AnyWrapper | undefined;
  if (!found) throw new Error(`no select ${id}`);
  return { items: () => (found.props("items") as { value: string }[]).map(i => i.value), text: () => found.get(`[data-testid="${id}"]`).text(), value: () => found.props("modelValue"), choose: async (v: string) => { found.vm.$emit("update:modelValue", v); await nextTick(); } };
}

/** The tag field: the input carries the test id, the tags sit beside it in the root. */
const tagsOf = (w: VueWrapper) => w.findAllComponents(UInputTags).find(t => t.find('[data-testid="field-tags"]').exists())!;

it("maps each descriptor type to a control holding the loaded value", async () => {
  const w = render(value());
  await flushPromises(); // the date picker loads lazily
  const val = (id: string) => (w.get(`[data-testid="${id}"]`).element as HTMLInputElement).value;
  expect(val("field-title")).toBe("Hi");
  expect(w.get('[data-testid="field-pubDate"]').text()).toBe("Jan 2, 2026");
  expect(select(w, "field-kind").value()).toBe("news");
  expect(w.get('[data-testid="field-featured"]').attributes("aria-checked")).toBe("true");
  expect(val("field-order")).toBe("3");
  expect(tagsOf(w).findAll('[data-slot="itemText"]').map(t => t.text())).toEqual(["a"]);
  expect(w.get('[data-testid="field-seo.noindex"]').attributes("aria-checked")).toBe("false");
  expect(select(w, "field-section").value()).toBe("hero");
  expect(val("field-section.heading")).toBe("H");
  expect(w.find('[data-testid="field-section.label"]').exists()).toBe(false);
  expect(val("field-related.0")).toBe("welcome");
  expect(w.get('[data-testid="field-coverImage-uploader"] img').attributes("src")).toBe("src/assets/blog/x.png");
  expect(w.get('[data-testid="field-custom-readonly"]').text()).toContain("Unknown custom refinement");
});

it("emits typed values for edits, array changes, variant switches, and cleared optionals", async () => {
  const w = render(value());
  await w.get('[data-testid="field-order"]').setValue("7");
  await w.get('[data-testid="field-featured"]').trigger("click");
  await select(w, "field-kind").choose("guide");
  const tags = w.get('input[data-testid="field-tags"]');
  await tags.setValue("b, c");
  await tags.trigger("keydown", { key: "Enter" });
  await tagsOf(w).findAll('[data-slot="item"]').find(t => t.text() === "a")!.get('[data-slot="itemDelete"]').trigger("click");
  await select(w, "field-section").choose("cta");
  await w.get('[data-testid="field-section.label"]').setValue("Go");
  await w.get('[data-testid="field-coverImage-uploader"] [data-slot="image-uploader-remove"]').trigger("click");
  w.findComponent(DatePicker).vm.$emit("update:modelValue", "2026-03-04");
  await nextTick();
  const out = w.props("modelValue") as Record<string, unknown>;
  expect(out).toMatchObject({ order: 7, featured: false, kind: "guide", tags: ["b", "c"], pubDate: "2026-03-04", section: { type: "cta", label: "Go" } });
  expect("coverImage" in out).toBe(false);
});

it("adds tags on Enter or blur, skips duplicates, and removes the last one on Backspace", async () => {
  // The tags field (Nuxt UI `UInputTags`) keeps the behaviour of the old tag input.
  const w = render(value());
  const tags = () => (w.props("modelValue") as { tags: string[] }).tags;
  const input = w.get('input[data-testid="field-tags"]');
  await input.setValue("a");
  await input.trigger("keydown", { key: "Enter" }); await flushPromises();
  expect(tags()).toEqual(["a"]);
  await input.setValue("b");
  await input.trigger("blur"); await flushPromises();
  expect(tags()).toEqual(["a", "b"]);
  await input.trigger("keydown", { key: "Backspace" }); await flushPromises();
  await input.trigger("keydown", { key: "Backspace" }); await flushPromises();
  expect(tags()).toEqual(["a"]);
});

it("an optional date picker offers Clear", async () => {
  const w = mount(DatePicker, { props: { modelValue: "2026-01-02", clearable: true }, attachTo: document.body });
  await w.get("button").trigger("click");
  await flushPromises();
  const clear = document.querySelector<HTMLButtonElement>('[data-testid="date-picker-clear"]');
  expect(document.querySelector('[data-testid="date-picker-calendar"]')).not.toBeNull();
  clear!.click();
  expect(w.emitted("update:modelValue")).toEqual([[undefined]]);
  w.unmount();
});

it("shows validation messages under the field path", () => {
  const w = mount(FrontmatterForm, { props: { descriptor, modelValue: value(), errors: { "section.heading": "Required", title: "Too short" } } });
  expect(w.get('[data-testid="field-title-error"]').text()).toBe("title: Too short");
  expect(w.get('[data-testid="field-section.heading-error"]').text()).toContain("section.heading: Required");
});

it("offers existing target entries for reference fields and keeps a missing value visible", async () => {
  const loaded = reactive<Record<string, string[]>>({});
  const load = vi.fn((collection: string) => { loaded[collection] = ["welcome", "launch-week"]; });
  const w = mount(FrontmatterForm, {
    props: { descriptor, modelValue: { ...value(), related: ["welcome", "", "gone"] }, errors: {}, "onUpdate:modelValue": (v: unknown) => w.setProps({ modelValue: v }) },
    global: { provide: { [REFERENCE_ENTRIES as symbol]: { entries: (c: string) => loaded[c], load } } },
  });
  await w.vm.$nextTick();
  expect(load).toHaveBeenCalledWith("blog");
  expect(select(w, "field-related.0").value()).toBe("welcome");
  expect(select(w, "field-related.1").value()).toBeUndefined();
  expect(select(w, "field-related.1").text()).toBe("Choose a blog entry");
  expect(select(w, "field-related.2").value()).toBe("gone");
  expect(select(w, "field-related.0").items()).toEqual(["welcome", "launch-week"]);
  expect(select(w, "field-related.2").items()).toEqual(["welcome", "launch-week", "gone"]);
  await select(w, "field-related.1").choose("launch-week");
  expect((w.props("modelValue") as { related: string[] }).related).toEqual(["welcome", "launch-week", "gone"]);
});
