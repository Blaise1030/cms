import { computed, ref } from "vue";

/**
 * Back/forward stack for the preview frame. The frame's own `history` is shared with the app's
 * top-level session history, so `history.back()` on the first frame page would navigate the app
 * away; this tracks what the frame has visited so the buttons only go where the frame can.
 */
export function useFrameHistory() {
  const entries = ref<string[]>([]);
  const index = ref(-1);
  let pending: "back" | "forward" | null = null;

  const current = computed(() => entries.value[index.value] ?? "/");
  const canGoBack = computed(() => index.value > 0);
  const canGoForward = computed(() => index.value < entries.value.length - 1);

  /** Record that the frame is now showing `path` (after a load or same-document navigation). */
  function visit(path: string) {
    const step = pending;
    pending = null;
    if (step === "back" && index.value > 0) {
      index.value--;
      entries.value[index.value] = path;
      return;
    }
    if (step === "forward" && index.value < entries.value.length - 1) {
      index.value++;
      entries.value[index.value] = path;
      return;
    }
    if (entries.value[index.value] === path) return;
    entries.value = [...entries.value.slice(0, index.value + 1), path];
    index.value = entries.value.length - 1;
  }

  /** Returns false when there is nowhere to go, so the caller must not touch `history`. */
  function back(): boolean {
    if (!canGoBack.value) return false;
    pending = "back";
    return true;
  }

  function forward(): boolean {
    if (!canGoForward.value) return false;
    pending = "forward";
    return true;
  }

  function reset() {
    entries.value = [];
    index.value = -1;
    pending = null;
  }

  return { entries, index, current, canGoBack, canGoForward, visit, back, forward, reset };
}
