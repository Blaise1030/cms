// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { DOMWrapper, flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import SendBackDialog from "./SendBackDialog.vue";

let mounted: VueWrapper | null = null;
afterEach(() => {
  mounted?.unmount();
  mounted = null;
  document.body.innerHTML = "";
});

async function mountOpen() {
  mounted = mount(SendBackDialog, { props: { open: true, busy: false }, attachTo: document.body });
  await flushPromises();
  return { w: mounted, body: new DOMWrapper(document.body) };
}

describe("SendBackDialog", () => {
  it("keeps Send back disabled until the comment has non-whitespace text", async () => {
    const { w, body } = await mountOpen();
    const submit = () => body.get("[data-testid='send-back-submit']");
    expect(submit().attributes("disabled")).toBeDefined();
    await body.get("[data-testid='send-back-comment']").setValue("   \n ");
    expect(submit().attributes("disabled")).toBeDefined();
    await body.get("[data-testid='send-back-comment']").setValue("Fix the heading");
    expect(submit().attributes("disabled")).toBeUndefined();
    await body.get("form").trigger("submit");
    expect(w.emitted("submit")?.[0]).toEqual(["Fix the heading"]);
  });
});
