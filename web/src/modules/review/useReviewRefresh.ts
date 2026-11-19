import { onBeforeUnmount, onMounted, watch } from "vue";

const REFRESH_MS = 60_000;
/** Focus and visibility changes within this long of the last refresh are ignored. */
export const WAKE_GAP_MS = 10_000;

/**
 * Run `refresh` now, whenever `source` changes, every minute while the page is visible, and when
 * the window comes back (focus or visibility) unless it ran in the last 10 s, so the focus and
 * visibilitychange that one tab switch fires cost one request, not two.
 */
export function useReviewRefresh(refresh: () => void, source?: () => unknown) {
  let last = -Infinity;
  const run = () => {
    last = Date.now();
    refresh();
  };
  if (source) watch(source, run);
  const visible = () => document.visibilityState !== "hidden";
  const tick = () => {
    if (visible()) run();
  };
  const wake = () => {
    if (visible() && Date.now() - last >= WAKE_GAP_MS) run();
  };
  let timer: ReturnType<typeof setInterval> | undefined;
  onMounted(() => {
    run();
    timer = setInterval(tick, REFRESH_MS);
    window.addEventListener("focus", wake);
    document.addEventListener("visibilitychange", wake);
  });
  onBeforeUnmount(() => {
    clearInterval(timer);
    window.removeEventListener("focus", wake);
    document.removeEventListener("visibilitychange", wake);
  });
}
