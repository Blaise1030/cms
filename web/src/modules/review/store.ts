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
  const inboxError = ref<string | null>(null);
  const openError = ref<string | null>(null);
  const gone = ref(false);
  /** The project the held state belongs to. */
  const projectId = ref<string | null>(null);
  let openRequest = 0;
  let loadRequest = 0;

  /** Switching project drops everything held for the old one. */
  function scope(id: string) {
    if (projectId.value === id) return;
    projectId.value = id;
    openRequest++;
    loadRequest++;
    items.value = [];
    current.value = null;
    diffText.value = null;
    diff.value = null;
    inboxError.value = null;
    openError.value = null;
    gone.value = false;
    loading.value = false;
    opening.value = false;
  }

  const waitingCount = computed(() => items.value.filter((i) => i.myState === "new").length);

  async function load(id: string) {
    scope(id);
    const request = ++loadRequest;
    loading.value = true;
    try {
      const list = await daemon.api.reviews(id);
      if (request !== loadRequest) return;
      items.value = list;
      inboxError.value = null;
    } catch (e) {
      if (request === loadRequest) inboxError.value = errorMessage(e);
    } finally {
      if (request === loadRequest) loading.value = false;
    }
  }

  async function open(id: string, number: number) {
    scope(id);
    const request = ++openRequest;
    opening.value = true;
    current.value = null;
    diffText.value = null;
    diff.value = null;
    openError.value = null;
    gone.value = false;
    try {
      // Lazy: the parser lives in `@pierre/diffs`, kept out of the app shell with the renderer.
      const [{ parseUnifiedDiff }, detail, text] = await Promise.all([
        import("../checkpoints/diff"),
        daemon.api.review(id, number),
        daemon.api.reviewDiff(id, number),
      ]);
      if (request !== openRequest) return;
      current.value = detail;
      diffText.value = text;
      diff.value = parseUnifiedDiff(text);
    } catch (e) {
      if (request !== openRequest) return;
      gone.value = (e as { status?: number }).status === 404;
      openError.value = errorMessage(e);
    } finally {
      if (request === openRequest) opening.value = false;
    }
  }

  return { items, loading, waitingCount, current, diffText, diff, opening, inboxError, openError, gone, projectId, load, open };
});
