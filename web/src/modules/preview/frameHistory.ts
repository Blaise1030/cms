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
    // A late report from the page being left (the frame is cross-origin, so reports arrive
    // asynchronously) is not the step's destination: keep waiting for that.
    if (pending && entries.value[index.value] === path) return;
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

  /** The path `step` entries away from the current one, or null when there is nowhere to go. */
  function target(step: -1 | 1): string | null {
    if (step < 0 ? !canGoBack.value : !canGoForward.value) return null;
    return entries.value[index.value + step] ?? null;
  }

  /** Where a back/forward step that has not landed yet is going, or null. */
  function destination(): string | null {
    if (pending === "back") return entries.value[index.value - 1] ?? null;
    if (pending === "forward") return entries.value[index.value + 1] ?? null;
    return null;
  }

  function reset() {
    entries.value = [];
    index.value = -1;
    pending = null;
  }

  return { entries, index, current, canGoBack, canGoForward, visit, back, forward, target, destination, reset };
}
