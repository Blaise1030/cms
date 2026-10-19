import { computed, readonly, ref, watchEffect } from "vue";
import { createSharedComposable, usePreferredDark } from "@vueuse/core";
import { readPref, writePref } from "@orso/utils";

export type ThemePreference = "light" | "dark" | "system";

/** Also read by the inline script in index.html, which applies the theme before the app loads. */
export const THEME_KEY = "orso.theme";

export const THEME_LABELS: Record<ThemePreference, string> = { light: "Light", dark: "Dark", system: "System" };

function parse(value: string | null): ThemePreference {
  return value === "light" || value === "dark" ? value : "system";
}

/** The app theme, applied as the `dark` class on `<html>` so portalled dialogs and menus follow it. */
export const useTheme = createSharedComposable(() => {
  const preference = ref<ThemePreference>(parse(readPref(THEME_KEY)));
  const prefersDark = usePreferredDark();
  const resolved = computed<"light" | "dark">(() =>
    preference.value === "system" ? (prefersDark.value ? "dark" : "light") : preference.value,
  );

  watchEffect(() => document.documentElement.classList.toggle("dark", resolved.value === "dark"));

  function setPreference(next: ThemePreference) {
    preference.value = next;
    writePref(THEME_KEY, next === "system" ? null : next);
  }

  return { preference: readonly(preference), resolved, setPreference };
});
