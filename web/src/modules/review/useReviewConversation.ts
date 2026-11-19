import { computed, ref, watch } from "vue";
import { useDaemon } from "@/core/daemon";
import type { ReviewConversation } from "@/core/types";
import { errorMessage } from "@orso/utils";

/**
 * Change `number`'s conversation, read again when the change or its head moves, and a way to
 * comment on it (appended on success, so the reviewer sees it without a reload).
 */
export function useReviewConversation(
  projectId: () => string,
  number: () => number,
  headSha: () => string | undefined,
) {
  const daemon = useDaemon();
  const conversation = ref<ReviewConversation | null>(null);
  const error = ref<string | null>(null);
  const loading = ref(false);
  const posting = ref(false);
  const postError = ref<string | null>(null);
  let run = 0;

  async function load() {
    const mine = ++run;
    loading.value = true;
    try {
      const next = await daemon.api.reviewConversation(projectId(), number());
      if (mine !== run) return;
      conversation.value = next;
      error.value = null;
    } catch (e) {
      if (mine !== run) return;
      error.value = errorMessage(e);
    } finally {
      if (mine === run) loading.value = false;
    }
  }

  /** Posts `body`; true when it went through. */
  async function comment(body: string): Promise<boolean> {
    if (!body.trim() || posting.value) return false;
    posting.value = true;
    postError.value = null;
    try {
      const item = await daemon.api.commentOnReview(projectId(), number(), body);
      if (conversation.value) conversation.value = { ...conversation.value, items: [item, ...conversation.value.items] };
      return true;
    } catch (e) {
      postError.value = errorMessage(e);
      return false;
    } finally {
      posting.value = false;
    }
  }

  /** What the Conversation tab counts: comments, and reviews that say something. */
  const count = computed(
    () =>
      conversation.value?.items.filter((i) => i.kind === "comment" || (i.kind === "review" && i.body)).length ??
      null,
  );

  // A list of sources compares each value, so a re-read change with the same head doesn't reload.
  watch(
    [projectId, number, headSha],
    ([, , sha], old) => {
      // Opening another change starts empty; a moved head refreshes in place.
      if (!old || old[0] !== projectId() || old[1] !== number()) conversation.value = null;
      if (sha) void load();
    },
    { immediate: true },
  );

  return { conversation, error, loading, posting, postError, count, reload: load, comment };
}
