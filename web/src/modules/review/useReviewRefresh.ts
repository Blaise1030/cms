import { onBeforeUnmount, onMounted } from "vue";

const REFRESH_MS = 60_000;

/** Run `refresh` now, when the window regains focus, and every minute while the page is visible. */
export function useReviewRefresh(refresh: () => void) {
  const visible = () => document.visibilityState !== "hidden";
  const tick = () => {
    if (visible()) refresh();
  };
  let timer: ReturnType<typeof setInterval> | undefined;
  onMounted(() => {
    refresh();
    timer = setInterval(tick, REFRESH_MS);
    window.addEventListener("focus", tick);
    document.addEventListener("visibilitychange", tick);
  });
  onBeforeUnmount(() => {
    clearInterval(timer);
    window.removeEventListener("focus", tick);
    document.removeEventListener("visibilitychange", tick);
  });
}
