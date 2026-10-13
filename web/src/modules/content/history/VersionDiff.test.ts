// @vitest-environment happy-dom
import { expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import VersionDiff from "./VersionDiff.vue";

const reason = "This version can't be restored because it couldn't be parsed.";

const mountDiff = () =>
  mount(VersionDiff, {
    props: { path: "a.md", oldSource: "x\n", newSource: "y\n", dirty: false, canRestore: false, restoreReason: reason },
    global: { stubs: { DiffView: true } },
  });

it("disables Restore and describes it with the full reason", () => {
  const wrapper = mountDiff();
  const restore = wrapper.get('[data-testid="restore-version"]');
  expect(restore.attributes("disabled")).toBeDefined();
  const describedBy = restore.attributes("aria-describedby");
  expect(describedBy).toBeTruthy();
  expect(wrapper.get(`#${describedBy}`).text()).toBe(reason);
});

it("emits back from Back to editor", async () => {
  const wrapper = mountDiff();
  await wrapper.findAll("button").find((b) => b.text() === "Back to editor")!.trigger("click");
  expect(wrapper.emitted("back")).toHaveLength(1);
});
