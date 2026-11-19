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
  /** Closed and merged changes, loaded on demand; `null` until then. */
  const closedItems = ref<ReviewItem[] | null>(null);
  const closedMore = ref(false);
  const closedLoading = ref(false);
  const closedError = ref<string | null>(null);
  let closedRequest = 0;
  const current = ref<ReviewDetail | null>(null);
  const diffText = ref<string | null>(null);
  const diff = ref<ParsedDiff | null>(null);
  const opening = ref(false);
  /** An in-place reload of the open change, and why it failed. */
  const refreshing = ref(false);
  const refreshError = ref<string | null>(null);
  /** The inbox's or the open change's failure; `gone` marks a change that left the inbox (404). */
  const inboxError = ref<string | null>(null);
  const openError = ref<string | null>(null);
  const gone = ref(false);
  /** A decision in flight, and why the last one did not go through. */
  const deciding = ref(false);
  const decideError = ref<string | null>(null);
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
    closedItems.value = null;
    closedMore.value = false;
    closedLoading.value = false;
    closedError.value = null;
    closedRequest++;
    current.value = null;
    diffText.value = null;
    diff.value = null;
    inboxError.value = null;
    openError.value = null;
    gone.value = false;
    deciding.value = false;
    decideError.value = null;
    loading.value = false;
    opening.value = false;
    refreshing.value = false;
    refreshError.value = null;
  }

  const waitingCount = computed(() => items.value.filter((i) => !i.mine && i.myState === "new").length);

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

  async function loadClosed(id: string) {
    scope(id);
    const request = ++closedRequest;
    closedLoading.value = true;
    try {
      const { reviews, more } = await daemon.api.closedReviews(id);
      if (request !== closedRequest) return;
      closedItems.value = reviews;
      closedMore.value = more;
      closedError.value = null;
    } catch (e) {
      if (request === closedRequest) closedError.value = errorMessage(e);
    } finally {
      if (request === closedRequest) closedLoading.value = false;
    }
  }

  /** Open change `number`; when the diff's head differs from the detail's, reload once. */
  function open(id: string, number: number) {
    return openChange(id, number, true);
  }

  /**
   * Read the open change again in place: detail and diff are swapped when both arrive, so the
   * page stays up; a failure shows in `refreshError` (a change that left becomes `gone`).
   */
  function refresh(id: string, number: number) {
    return openChange(id, number, true, true);
  }

  async function openChange(id: string, number: number, recheck: boolean, keep = false): Promise<void> {
    scope(id);
    const request = ++openRequest;
    refreshError.value = null;
    if (keep) {
      refreshing.value = true;
    } else {
      opening.value = true;
      current.value = null;
      diffText.value = null;
      diff.value = null;
      openError.value = null;
      decideError.value = null;
      gone.value = false;
    }
    try {
      // Lazy: the parser lives in `@pierre/diffs`, kept out of the app shell with the renderer.
      const [{ parseUnifiedDiff }, detail, { text, headSha }] = await Promise.all([
        import("../checkpoints/diff"),
        daemon.api.review(id, number),
        daemon.api.reviewDiff(id, number),
      ]);
      if (request !== openRequest) return;
      // The change moved between the two requests: the diff and the detail would disagree.
      if (recheck && headSha && headSha.toLowerCase() !== detail.headSha.toLowerCase()) {
        await openChange(id, number, false, keep);
        return;
      }
      current.value = detail;
      diffText.value = text;
      diff.value = parseUnifiedDiff(text);
    } catch (e) {
      if (request !== openRequest) return;
      gone.value = (e as { status?: number }).status === 404;
      if (keep && !gone.value) refreshError.value = errorMessage(e);
      else openError.value = errorMessage(e);
    } finally {
      if (request === openRequest) {
        opening.value = false;
        refreshing.value = false;
      }
    }
  }

  /**
   * Greenlight or send back the open change. The row and the open change take the new state
   * without a reload. `head_moved` reloads detail and diff and says why; other failures show
   * their message. Resolves true when the decision went through, or "head_moved" when the change moved.
   */
  async function decide(decision: "greenlight" | "send_back", comment?: string): Promise<boolean | "head_moved"> {
    const detail = current.value;
    const id = projectId.value;
    if (!detail || !id || deciding.value) return false;
    deciding.value = true;
    decideError.value = null;
    try {
      await daemon.api.decideReview(id, detail.number, { decision, comment, headSha: detail.headSha });
      if (projectId.value !== id) return false;
      const myState = decision === "greenlight" ? "greenlit" : "sent_back";
      for (const item of items.value) if (item.number === detail.number) item.myState = myState;
      if (current.value?.number === detail.number) current.value.myState = myState;
      return true;
    } catch (e) {
      if (projectId.value !== id) return false;
      const err = e as { status?: number; reason?: string };
      if (err.status === 409 && err.reason === "head_moved") {
        await open(id, detail.number);
        decideError.value = "This change was updated since you opened it";
        return "head_moved";
      } else {
        if (err.status === 404) gone.value = true;
        decideError.value = errorMessage(e);
      }
      return false;
    } finally {
      deciding.value = false;
    }
  }

  return { refresh, refreshing, refreshError, closedItems, closedMore, closedLoading, closedError, loadClosed, deciding, decideError, decide, items, loading, waitingCount, current, diffText, diff, opening, inboxError, openError, gone, projectId, load, open };
});
