// @vitest-environment happy-dom
import { expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import FrontmatterForm from "./FrontmatterForm.vue";
import type { FieldDescriptor } from "@/core/types";

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

it("maps each descriptor type to a control holding the loaded value", () => {
  const w = render(value());
  const val = (id: string) => (w.get(`[data-testid="${id}"]`).element as HTMLInputElement).value;
  expect(val("field-title")).toBe("Hi");
  expect(w.get('[data-testid="field-pubDate"]').attributes("type")).toBe("date");
  expect(val("field-pubDate")).toBe("2026-01-02");
  expect(val("field-kind")).toBe("news");
  expect((w.get('[data-testid="field-featured"]').element as HTMLInputElement).checked).toBe(true);
  expect(val("field-order")).toBe("3");
  expect(val("field-tags.0")).toBe("a");
  expect((w.get('[data-testid="field-seo.noindex"]').element as HTMLInputElement).checked).toBe(false);
  expect(val("field-section")).toBe("hero");
  expect(val("field-section.heading")).toBe("H");
  expect(w.find('[data-testid="field-section.label"]').exists()).toBe(false);
  expect(val("field-related.0")).toBe("welcome");
  expect(val("field-coverImage")).toBe("src/assets/blog/x.png");
  expect(w.get('[data-testid="field-custom-readonly"]').text()).toContain("Unknown custom refinement");
});

it("emits typed values for edits, array changes, variant switches, and cleared optionals", async () => {
  const w = render(value());
  await w.get('[data-testid="field-order"]').setValue("7");
  await w.get('[data-testid="field-featured"]').setValue(false);
  await w.get('[data-testid="field-kind"]').setValue("guide");
  await w.get('[data-testid="field-tags-add"]').trigger("click");
  await w.get('[data-testid="field-tags.1"]').setValue("b");
  await w.get('[data-testid="field-tags-remove-0"]').trigger("click");
  await w.get('[data-testid="field-section"]').setValue("cta");
  await w.get('[data-testid="field-section.label"]').setValue("Go");
  await w.get('[data-testid="field-coverImage"]').setValue("");
  const out = w.props("modelValue") as Record<string, unknown>;
  expect(out).toMatchObject({ order: 7, featured: false, kind: "guide", tags: ["b"], section: { type: "cta", label: "Go" } });
  expect("coverImage" in out).toBe(false);
});

it("shows validation messages under the field path", () => {
  const w = mount(FrontmatterForm, { props: { descriptor, modelValue: value(), errors: { "section.heading": "Required", title: "Too short" } } });
  expect(w.get('[data-testid="field-title-error"]').text()).toBe("title: Too short");
  expect(w.get('[data-testid="field-section.heading-error"]').text()).toContain("section.heading: Required");
});
