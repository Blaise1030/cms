import { computed, readonly, ref } from "vue";
import { createSharedComposable } from "@vueuse/core";
import { readPref, writePref } from "@orso/utils";

/** How the agent chat is presented: docked column vs Linear-style floating + bottom bar. */
export type ChatPanelLayout = "sidebar" | "floating";

export const PANEL_LAYOUT_KEY = "orso.chat.panelLayout";

function parse(value: string | null): ChatPanelLayout {
  return value === "floating" ? "floating" : "sidebar";
}

/** Agent panel layout preference. Sidebar by default. */
export const useChatPanelLayout = createSharedComposable(() => {
  const layout = ref<ChatPanelLayout>(parse(readPref(PANEL_LAYOUT_KEY)));
  const floating = computed(() => layout.value === "floating");

  function set(next: ChatPanelLayout) {
    layout.value = next;
    writePref(PANEL_LAYOUT_KEY, next === "floating" ? "floating" : null);
  }

  return { layout: readonly(layout), floating, set };
});
