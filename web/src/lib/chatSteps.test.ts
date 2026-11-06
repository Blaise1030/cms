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
  const { useChatSteps, SHOW_STEPS_KEY } = await import("./chatSteps");
  const scope = effectScope();
  const steps = scope.run(() => useChatSteps())!;
  await nextTick();
  return { steps, SHOW_STEPS_KEY, scope };
}

it("hides assistant steps by default", async () => {
  const { steps } = await load();
  expect(steps.enabled.value).toBe(false);
});

it("remembers an explicit on/off choice", async () => {
  const { steps, SHOW_STEPS_KEY } = await load();
  steps.set(true);
  await nextTick();
  expect(steps.enabled.value).toBe(true);
  expect(localStorage.getItem(SHOW_STEPS_KEY)).toBe("1");

  steps.set(false);
  await nextTick();
  expect(steps.enabled.value).toBe(false);
  expect(localStorage.getItem(SHOW_STEPS_KEY)).toBeNull();
});

it("restores a stored on choice and ignores junk", async () => {
  localStorage.setItem("orso.chat.showSteps", "1");
  expect((await load()).steps.enabled.value).toBe(true);

  vi.resetModules();
  localStorage.setItem("orso.chat.showSteps", "maybe");
  expect((await load()).steps.enabled.value).toBe(false);
});
