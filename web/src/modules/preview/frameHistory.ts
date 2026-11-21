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
  /** A page the app sent the frame to (typed address, "open in preview"), and the page left. */
  let expected: { path: string; leaving: string | null } | null = null;

  const current = computed(() => entries.value[index.value] ?? "/");
  const canGoBack = computed(() => index.value > 0);
  const canGoForward = computed(() => index.value < entries.value.length - 1);

  /**
   * Record that the frame is now showing `path` (after a load or same-document navigation).
   * Returns false for a report it ignores (a late one from the page being left).
   */
  function visit(path: string): boolean {
    if (expected) {
      // The page being left can still report itself after the frame was sent elsewhere (the
      // frame is cross-origin, and its window is the same across navigations, so the report
      // looks like the new page's). Ignore it, or the next reload would go back there.
      if (path === expected.leaving && path !== expected.path) return false;
      const asked = expected.path;
      expected = null;
      if (path === asked) return true;
      // Anything else is where the requested page led (a redirect): it replaces the request.
      if (entries.value[index.value] === asked) {
        entries.value[index.value] = path;
        return true;
      }
    }
    // A late report from the page being left (the frame is cross-origin, so reports arrive
    // asynchronously) is not the step's destination: keep waiting for that.
    if (pending && entries.value[index.value] === path) return false;
    const step = pending;
    pending = null;
    if (step === "back" && index.value > 0) {
      index.value--;
      entries.value[index.value] = path;
      return true;
    }
    if (step === "forward" && index.value < entries.value.length - 1) {
      index.value++;
      entries.value[index.value] = path;
      return true;
    }
    if (entries.value[index.value] === path) return true;
    entries.value = [...entries.value.slice(0, index.value + 1), path];
    index.value = entries.value.length - 1;
    return true;
  }

  /**
   * The app is sending the frame to `path`: show it now, and until the frame reports it (or
   * where it redirected), ignore late reports from `leaving`, the page it is on now.
   */
  function navigate(path: string, leaving: string | null = entries.value[index.value] ?? null) {
    pending = null;
    expected = { path, leaving };
    if (entries.value[index.value] === path) return;
    entries.value = [...entries.value.slice(0, index.value + 1), path];
    index.value = entries.value.length - 1;
  }

  /** Returns false when there is nowhere to go, so the caller must not touch `history`. */
  function back(): boolean {
    if (!canGoBack.value) return false;
    // A step replaces any page the app asked for that has not reported in yet.
    expected = null;
    pending = "back";
    return true;
  }

  function forward(): boolean {
    if (!canGoForward.value) return false;
    expected = null;
    pending = "forward";
    return true;
  }

  /** The path `step` entries away from the current one, or null when there is nowhere to go. */
  function target(step: -1 | 1): string | null {
    if (step < 0 ? !canGoBack.value : !canGoForward.value) return null;
    return entries.value[index.value + step] ?? null;
  }

  /** The page the app sent the frame to that has not reported in yet, or null. */
  function awaiting(): string | null {
    return expected?.path ?? null;
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
    expected = null;
  }

  return { entries, index, current, canGoBack, canGoForward, visit, navigate, awaiting, back, forward, target, destination, reset };
}
