import { readonly, ref } from "vue";
import { createSharedComposable } from "@vueuse/core";
import { readPref, writePref } from "@orso/utils";

/** When set to `"0"`, the content panel stops warning about collections it can't edit. Missing means shown (default). */
export const HIDDEN_COLLECTIONS_NOTICE_KEY = "orso.content.hiddenCollectionsNotice";

function parse(value: string | null): boolean {
  return value !== "0" && value !== "false";
}

/**
 * Whether the content panel warns about collections it leaves out of the tree (for example the
 * Crowdin-owned `*I18n` translations). On by default; the collections stay hidden either way.
 */
export const useHiddenCollectionsNotice = createSharedComposable(() => {
  const enabled = ref(parse(readPref(HIDDEN_COLLECTIONS_NOTICE_KEY)));

  function set(next: boolean) {
    enabled.value = next;
    writePref(HIDDEN_COLLECTIONS_NOTICE_KEY, next ? null : "0");
  }

  return { enabled: readonly(enabled), set };
});
