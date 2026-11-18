import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { useDaemon } from "@/core/daemon";
import type { ReviewDetail, ReviewItem } from "@/core/types";
import { errorMessage } from "@orso/utils";
import type { ParsedDiff } from "../checkpoints/diff";

export const useReviewStore = defineStore("review", () => {
  const daemon = useDaemon();

  const items = ref<ReviewItem[]>([]);
  const loading = ref(false);
  const current = ref<ReviewDetail | null>(null);
  const diffText = ref<string | null>(null);
  const diff = ref<ParsedDiff | null>(null);
  const opening = ref(false);
  /** The inbox's or the open change's failure; `gone` marks a change that left the inbox (404). */
  const error = ref<string | null>(null);
  const gone = ref(false);
  let openRequest = 0;

  const waitingCount = computed(() => items.value.filter((i) => i.myState === "new").length);

  async function load(projectId: string) {
    loading.value = true;
    try {
      items.value = await daemon.api.reviews(projectId);
      error.value = null;
    } catch (e) {
      error.value = errorMessage(e);
    } finally {
      loading.value = false;
    }
  }

  async function open(projectId: string, number: number) {
    const request = ++openRequest;
    opening.value = true;
    current.value = null;
    diffText.value = null;
    diff.value = null;
    error.value = null;
    gone.value = false;
    try {
      // Lazy: the parser lives in `@pierre/diffs`, kept out of the app shell with the renderer.
      const [{ parseUnifiedDiff }, detail, text] = await Promise.all([
        import("../checkpoints/diff"),
        daemon.api.review(projectId, number),
        daemon.api.reviewDiff(projectId, number),
      ]);
      if (request !== openRequest) return;
      current.value = detail;
      diffText.value = text;
      diff.value = parseUnifiedDiff(text);
    } catch (e) {
      if (request !== openRequest) return;
      gone.value = (e as { status?: number }).status === 404;
      error.value = errorMessage(e);
    } finally {
      if (request === openRequest) opening.value = false;
    }
  }

  return { items, loading, waitingCount, current, diffText, diff, opening, error, gone, load, open };
});
