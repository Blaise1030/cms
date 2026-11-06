// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { effectScope, nextTick } from "vue";

beforeEach(() => {
  vi.resetModules();
  localStorage.clear();
});
afterEach(() => {
  localStorage.clear();
});

async function load() {
  const { useChatPanelLayout, PANEL_LAYOUT_KEY } = await import("./chatPanelLayout");
  const scope = effectScope();
  const layout = scope.run(() => useChatPanelLayout())!;
  await nextTick();
  return { layout, PANEL_LAYOUT_KEY, scope };
}

it("defaults to the docked sidebar", async () => {
  const { layout } = await load();
  expect(layout.layout.value).toBe("sidebar");
  expect(layout.floating.value).toBe(false);
});

it("remembers sidebar vs floating", async () => {
  const { layout, PANEL_LAYOUT_KEY } = await load();
  layout.set("floating");
  await nextTick();
  expect(layout.layout.value).toBe("floating");
  expect(layout.floating.value).toBe(true);
  expect(localStorage.getItem(PANEL_LAYOUT_KEY)).toBe("floating");

  layout.set("sidebar");
  await nextTick();
  expect(layout.layout.value).toBe("sidebar");
  expect(layout.floating.value).toBe(false);
  expect(localStorage.getItem(PANEL_LAYOUT_KEY)).toBeNull();
});

it("restores a stored floating choice and ignores junk", async () => {
  localStorage.setItem("orso.chat.panelLayout", "floating");
  expect((await load()).layout.layout.value).toBe("floating");

  vi.resetModules();
  localStorage.setItem("orso.chat.panelLayout", "maybe");
  expect((await load()).layout.layout.value).toBe("sidebar");
});
