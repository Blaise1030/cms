// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import type { DesignTokens } from "@/core/types";
import DesignTokensForm from "./DesignTokensForm.vue";
import { addKey, removeKey, renameKey } from "./tokens";

describe("tokens", () => {
  it("renameKey keeps position", () => {
    const renamed = renameKey({ a: 1, b: 2, c: 3 }, "b", "x");
    expect(renamed).toEqual({ a: 1, x: 2, c: 3 });
    expect(Object.keys(renamed!)).toEqual(["a", "x", "c"]);
  });

  it("rename to an existing name is rejected", () => {
    expect(renameKey({ a: 1, b: 2 }, "a", "b")).toBeNull();
    expect(renameKey({ a: 1, b: 2 }, "a", "")).toBeNull();
  });

  it("addKey avoids clashes", () => {
    expect(addKey({}, "color", 1)).toEqual({ color: 1 });
    expect(Object.keys(addKey({ color: 1, "color-2": 2 }, "color", 3))).toEqual(["color", "color-2", "color-3"]);
  });

  it("removeKey drops only that key", () => {
    expect(removeKey({ a: 1, b: 2 }, "a")).toEqual({ b: 2 });
  });
});

/** Mounts the form, feeding each emitted value back in as the model (like v-model), and returns the last tokens emitted. */
function render(modelValue: DesignTokens) {
  const w = mount(DesignTokensForm, { props: { modelValue, "onUpdate:modelValue": (v: DesignTokens) => w.setProps({ modelValue: v }) } });
  const last = () => {
    const events = w.emitted<[DesignTokens]>("update:modelValue") ?? [];
    return events[events.length - 1]?.[0];
  };
  return { w, last };
}
const id = (testid: string) => `[data-testid="${testid}"]`;

describe("DesignTokensForm", () => {
  it("editing a colour emits tokens with other groups untouched", async () => {
    const input = { colors: { primary: "#111" }, components: { btn: {} }, version: "alpha" };
    const { w, last } = render(input);
    await w.get(id("design-colors-value-primary")).setValue("#222222");
    expect(last()).toEqual({ colors: { primary: "#222222" }, components: { btn: {} }, version: "alpha" });
    expect(input.colors.primary).toBe("#111");
  });

  it("shows a swatch, and #000000 in the picker when the value is not hex", () => {
    const { w } = render({ colors: { hex: "#ff0000", named: "rebeccapurple" } });
    expect(w.get(id("design-colors-picker-hex")).element).toHaveProperty("value", "#ff0000");
    expect(w.get(id("design-colors-picker-named")).element).toHaveProperty("value", "#000000");
    expect(w.get(id("design-colors-swatch-named")).attributes("style")).toContain("rebeccapurple");
  });

  it("picking a colour emits its hex", async () => {
    const { w, last } = render({ colors: { primary: "#111111" } });
    await w.get(id("design-colors-picker-primary")).setValue("#abcdef");
    expect(last()).toEqual({ colors: { primary: "#abcdef" } });
  });

  it("adding a colour appends a clash-free token", async () => {
    const { w, last } = render({ colors: { color: "#fff" } });
    await w.get(id("design-colors-add")).trigger("click");
    expect(Object.entries((last() as { colors: object }).colors)).toEqual([["color", "#fff"], ["color-2", "#000000"]]);
  });

  it("removing a spacing token", async () => {
    const { w, last } = render({ spacing: { sm: "4px", md: "8px" }, rounded: { sm: "2px" } });
    await w.get(id("design-spacing-remove-sm")).trigger("click");
    expect(last()).toEqual({ spacing: { md: "8px" }, rounded: { sm: "2px" } });
  });

  it("adding a spacing and a rounded token uses size / 8px", async () => {
    const { w, last } = render({});
    await w.get(id("design-spacing-add")).trigger("click");
    expect(last()).toEqual({ spacing: { size: "8px" } });
    await w.get(id("design-rounded-add")).trigger("click");
    expect(last()).toEqual({ spacing: { size: "8px" }, rounded: { size: "8px" } });
  });

  it("adding a typography style", async () => {
    const { w, last } = render({ typography: { body: { fontFamily: "Inter" } }, version: "alpha" });
    await w.get(id("design-typography-add")).trigger("click");
    expect(last()).toEqual({
      typography: { body: { fontFamily: "Inter" }, style: { fontFamily: "Inter", fontSize: "16px", fontWeight: 400, lineHeight: 1.5 } },
      version: "alpha",
    });
  });

  it("editing a typography field keeps the other fields", async () => {
    const { w, last } = render({ typography: { body: { fontFamily: "Inter", fontSize: "16px", fontWeight: 400, lineHeight: 1.5 } } });
    await w.get(id("design-typography-fontSize-body")).setValue("18px");
    await w.get(id("design-typography-fontWeight-body")).setValue("700");
    expect(last()).toEqual({ typography: { body: { fontFamily: "Inter", fontSize: "18px", fontWeight: 700, lineHeight: 1.5 } } });
  });

  it("empty letterSpacing is omitted", async () => {
    const { w, last } = render({ typography: { body: { fontFamily: "Inter", letterSpacing: "0.1em" } } });
    await w.get(id("design-typography-letterSpacing-body")).setValue("");
    const body = (last() as { typography: { body: object } }).typography.body;
    expect(body).toEqual({ fontFamily: "Inter" });
    expect("letterSpacing" in body).toBe(false);
  });

  it("emits fontWeight as a number", async () => {
    const { w, last } = render({ typography: { body: { fontWeight: 400 } } });
    await w.get(id("design-typography-fontWeight-body")).setValue("600");
    expect(last()).toEqual({ typography: { body: { fontWeight: 600 } } });
  });

  it("emits a unitless lineHeight as a number and a dimension as a string", async () => {
    const { w, last } = render({ typography: { body: { lineHeight: 1.5 } } });
    await w.get(id("design-typography-lineHeight-body")).setValue("1.6");
    expect(last()).toEqual({ typography: { body: { lineHeight: 1.6 } } });
    await w.get(id("design-typography-lineHeight-body")).setValue("24px");
    expect(last()).toEqual({ typography: { body: { lineHeight: "24px" } } });
    await w.get(id("design-typography-lineHeight-body")).setValue("1.5rem");
    expect(last()).toEqual({ typography: { body: { lineHeight: "1.5rem" } } });
  });

  it("an invalid fontWeight is not emitted and reverts on blur", async () => {
    const { w, last } = render({ typography: { body: { fontWeight: 400 } } });
    const input = w.get(id("design-typography-fontWeight-body"));
    await input.setValue("bold");
    expect(last()).toBeUndefined();
    expect((input.element as HTMLInputElement).value).toBe("bold");
    await input.trigger("blur");
    expect(last()).toBeUndefined();
    expect((input.element as HTMLInputElement).value).toBe("400");
  });

  it("an incomplete lineHeight is not emitted while typing, and completing it is", async () => {
    const { w, last } = render({ typography: { body: { lineHeight: 1 } } });
    const input = w.get(id("design-typography-lineHeight-body"));
    await input.setValue("1.");
    expect(last()).toBeUndefined();
    expect((input.element as HTMLInputElement).value).toBe("1.");
    await input.setValue("1.5");
    expect(last()).toEqual({ typography: { body: { lineHeight: 1.5 } } });
    await input.setValue("");
    expect(last()).toEqual({ typography: { body: { lineHeight: 1.5 } } });
    await input.trigger("blur");
    expect((input.element as HTMLInputElement).value).toBe("1.5");
  });

  it("a value set from outside replaces an in-progress draft", async () => {
    const { w } = render({ typography: { body: { lineHeight: 1 } } });
    const input = w.get(id("design-typography-lineHeight-body"));
    await input.setValue("1.");
    expect((input.element as HTMLInputElement).value).toBe("1.");
    await w.setProps({ modelValue: { typography: { body: { lineHeight: 2 } } } });
    expect((input.element as HTMLInputElement).value).toBe("2");
  });

  it("a value set from outside replaces an in-progress rename", async () => {
    const { w } = render({ colors: { a: "#111" } });
    await w.get(id("design-colors-name-a")).setValue("half");
    await w.setProps({ modelValue: { colors: { a: "#222" } } });
    expect((w.get(id("design-colors-name-a")).element as HTMLInputElement).value).toBe("a");
  });

  it("removing a style drops its drafts, so a re-added style starts clean", async () => {
    const { w } = render({ typography: { style: { lineHeight: 1 } } });
    await w.get(id("design-typography-lineHeight-style")).setValue("1.");
    await w.get(id("design-typography-remove-style")).trigger("click");
    await w.get(id("design-typography-add")).trigger("click");
    expect((w.get(id("design-typography-lineHeight-style")).element as HTMLInputElement).value).toBe("1.5");
  });

  it("edits the brand name and description", async () => {
    const { w, last } = render({ name: "Old", version: "alpha" });
    await w.get(id("design-brand-name")).setValue("New");
    expect(last()).toEqual({ name: "New", version: "alpha" });
    await w.get(id("design-brand-description")).setValue("Calm");
    expect(last()).toEqual({ name: "New", description: "Calm", version: "alpha" });
  });

  it("renames on blur, keeping the token's position", async () => {
    const { w, last } = render({ colors: { a: "#111", b: "#222", c: "#333" } });
    const name = w.get(id("design-colors-name-b"));
    await name.setValue("brand");
    expect(last()).toBeUndefined();
    await name.trigger("blur");
    expect(Object.keys((last() as { colors: object }).colors)).toEqual(["a", "brand", "c"]);
  });

  it("renames on Enter", async () => {
    const { w, last } = render({ spacing: { sm: "4px" } });
    await w.get(id("design-spacing-name-sm")).setValue("small");
    await w.get(id("design-spacing-name-sm")).trigger("keydown", { key: "Enter" });
    expect(last()).toEqual({ spacing: { small: "4px" } });
  });

  it("a name already used is restored and reported", async () => {
    const { w, last } = render({ colors: { a: "#111", b: "#222" } });
    const name = w.get(id("design-colors-name-a"));
    await name.setValue("b");
    await name.trigger("blur");
    expect(last()).toBeUndefined();
    expect((w.get(id("design-colors-name-a")).element as HTMLInputElement).value).toBe("a");
    expect(w.get(id("design-colors-error-a")).text()).toBe("Name already used");
  });

  it("renders every group collapsible", () => {
    const { w } = render({});
    for (const group of ["brand", "colors", "typography", "rounded", "spacing"]) expect(w.find(id(`design-group-${group}`)).exists()).toBe(true);
  });
});
