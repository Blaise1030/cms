// @vitest-environment happy-dom
import { expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import ImageUploader from "./ImageUploader.vue";

const png = () => new File([new Uint8Array([0x89])], "a.png", { type: "image/png" });

it("shows a drop zone when empty and emits picked and dropped files", async () => {
  const w = mount(ImageUploader);
  expect(w.find("img").exists()).toBe(false);
  expect(w.text()).toContain("Click to upload");

  const input = w.get('input[type="file"]');
  Object.defineProperty(input.element, "files", { value: [png()], configurable: true });
  await input.trigger("change");

  const drop = new Event("drop", { bubbles: true, cancelable: true });
  Object.defineProperty(drop, "dataTransfer", { value: { files: [png()] } });
  w.element.dispatchEvent(drop);

  expect(w.emitted("select")?.map(([f]) => (f as File).name)).toEqual(["a.png", "a.png"]);
});

it("previews the image with Replace and Remove, and falls back when it cannot load", async () => {
  const w = mount(ImageUploader, { props: { src: "/raw/a.png" } });
  expect(w.get("img").attributes("src")).toBe("/raw/a.png");
  await w.get('[aria-label="Remove image"]').trigger("click");
  expect(w.emitted("remove")).toHaveLength(1);

  await w.get("img").trigger("error");
  expect(w.find("img").exists()).toBe(false);
  expect(w.text()).toContain("Preview unavailable");
  await w.setProps({ src: "/raw/b.png" });
  expect(w.get("img").attributes("src")).toBe("/raw/b.png");
});
