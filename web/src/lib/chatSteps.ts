import { readonly, ref } from "vue";
import { createSharedComposable } from "@vueuse/core";
import { readPref, writePref } from "@orso/utils";

/** When set to `"1"`, chat shows “Worked through N steps”. Missing means hidden (default). */
export const SHOW_STEPS_KEY = "orso.chat.showSteps";

function parse(value: string | null): boolean {
  return value === "1" || value === "true";
}

/** Whether finished assistant step traces appear in chat. Off by default. */
export const useChatSteps = createSharedComposable(() => {
  const enabled = ref(parse(readPref(SHOW_STEPS_KEY)));

  function set(next: boolean) {
    enabled.value = next;
    writePref(SHOW_STEPS_KEY, next ? "1" : null);
  }

  return { enabled: readonly(enabled), set };
});
