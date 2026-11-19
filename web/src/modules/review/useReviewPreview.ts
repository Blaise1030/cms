import { onBeforeUnmount, ref, watch } from "vue";
import { useDaemon } from "@/core/daemon";
import type { ReviewPreview } from "@/core/types";
import { errorMessage } from "@orso/utils";

/** How often a deploy still running is checked again. */
export const DEPLOY_POLL_MS = 10_000;

/**
 * The deploy preview of change `number` in `projectId`, read again when the change or its head
 * changes. While the deploy runs it is checked every 10 s, but only while the page is visible:
 * a hidden tab checks once when it comes back.
 */
export function useReviewPreview(
  projectId: () => string,
  number: () => number,
  headSha: () => string | undefined,
) {
  const daemon = useDaemon();
  const preview = ref<ReviewPreview | null>(null);
  const error = ref<string | null>(null);
  const loading = ref(false);
  let run = 0;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let waitingForVisible = false;

  function stop() {
    if (timer) clearTimeout(timer);
    timer = null;
    waitingForVisible = false;
  }

  async function load() {
    stop();
    const mine = ++run;
    if (!headSha()) {
      preview.value = null;
      return;
    }
    loading.value = true;
    try {
      const next = await daemon.api.reviewPreview(projectId(), number());
      if (mine !== run) return;
      preview.value = next;
      error.value = null;
      if (next.state === "deploying") timer = setTimeout(() => poll(mine), DEPLOY_POLL_MS);
    } catch (e) {
      if (mine !== run) return;
      error.value = errorMessage(e);
    } finally {
      if (mine === run) loading.value = false;
    }
  }

  function poll(mine: number) {
    if (mine !== run) return;
    if (document.visibilityState === "hidden") waitingForVisible = true;
    else void load();
  }

  function onVisible() {
    if (waitingForVisible && document.visibilityState !== "hidden") {
      waitingForVisible = false;
      void load();
    }
  }
  document.addEventListener("visibilitychange", onVisible);

  // A list of sources compares each value, so a re-read change with the same head doesn't reload.
  watch(
    [projectId, number, headSha],
    () => {
      preview.value = null;
      error.value = null;
      void load();
    },
    { immediate: true },
  );
  onBeforeUnmount(() => {
    run++;
    stop();
    document.removeEventListener("visibilitychange", onVisible);
  });

  return { preview, error, loading, reload: load };
}
