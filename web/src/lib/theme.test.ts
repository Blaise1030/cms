// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { effectScope, nextTick } from "vue";

let systemDark = false;
const listeners = new Set<() => void>();

beforeEach(() => {
  vi.resetModules();
  localStorage.clear();
  document.documentElement.className = "";
  systemDark = false;
  listeners.clear();
  vi.stubGlobal("matchMedia", (query: string) => ({
    get matches() { return query.includes("dark") && systemDark; },
    media: query,
    addEventListener: (_: string, fn: () => void) => listeners.add(fn),
    removeEventListener: (_: string, fn: () => void) => listeners.delete(fn),
  }));
});
afterEach(() => vi.unstubAllGlobals());

async function load() {
  const { useTheme, THEME_KEY } = await import("./theme");
  const scope = effectScope();
  const theme = scope.run(() => useTheme())!;
  await nextTick();
  return { theme, THEME_KEY, scope };
}

const isDark = () => document.documentElement.classList.contains("dark");

it("follows the system by default", async () => {
  systemDark = true;
  const { theme } = await load();
  expect(theme.preference.value).toBe("system");
  expect(theme.resolved.value).toBe("dark");
  expect(isDark()).toBe(true);
});

it("applies and remembers an explicit choice", async () => {
  const { theme, THEME_KEY } = await load();
  theme.setPreference("dark");
  await nextTick();
  expect(isDark()).toBe(true);
  expect(localStorage.getItem(THEME_KEY)).toBe("dark");

  theme.setPreference("light");
  await nextTick();
  expect(isDark()).toBe(false);
  expect(localStorage.getItem(THEME_KEY)).toBe("light");

  theme.setPreference("system");
  await nextTick();
  expect(localStorage.getItem(THEME_KEY)).toBeNull();
});

it("restores the stored choice and ignores junk", async () => {
  localStorage.setItem("workbench.theme", "dark");
  expect((await load()).theme.preference.value).toBe("dark");

  vi.resetModules();
  localStorage.setItem("workbench.theme", "purple");
  expect((await load()).theme.preference.value).toBe("system");
});
