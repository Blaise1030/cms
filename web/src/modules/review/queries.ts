import { computed, reactive, ref } from "vue";
import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/vue-query";
import { useDaemon } from "@/core/daemon";
import type { ReviewConversation, ReviewDetail, ReviewItem, ReviewPreview, TimelineItem } from "@/core/types";
import { errorMessage } from "@orso/utils";
import type { ParsedDiff } from "../checkpoints/diff";

/**
 * The review module's server state, held in TanStack Query: data is shared between the rail,
 * the inbox and a change, fresh for 30 s, read again when the window regains focus, and polled
 * only while the page is visible.
 */

/** Data younger than this is used as is. */
export const STALE_MS = 30_000;
/** The inbox (and the rail's badge) is read again this often while visible. */
export const INBOX_POLL_MS = 60_000;
/** A deploy still running is checked this often while visible. */
export const DEPLOY_POLL_MS = 10_000;

export const reviewKeys = {
  all: (project: string) => ["review", project] as const,
  inbox: (project: string) => ["review", project, "inbox"] as const,
  closed: (project: string) => ["review", project, "closed"] as const,
  change: (project: string, n: number) => ["review", project, "change", n] as const,
  conversation: (project: string, n: number) => ["review", project, "change", n, "conversation"] as const,
  preview: (project: string, n: number, sha: string | undefined) =>
    ["review", project, "change", n, "preview", sha ?? ""] as const,
};

const status = (e: unknown) => (e as { status?: number } | null)?.status;
/** Failures show at once with their own Try again / refresh; nothing retries behind them. */
const common = { staleTime: STALE_MS, refetchOnWindowFocus: true, retry: false } as const;

/** Open changes waiting in `projectId`'s inbox, and how many wait on you. */
export function useInbox(projectId: () => string) {
  const daemon = useDaemon();
  const q = useQuery({
    ...common,
    queryKey: computed(() => reviewKeys.inbox(projectId())),
    queryFn: () => daemon.api.reviews(projectId()),
    refetchInterval: INBOX_POLL_MS,
  });
  const items = computed<ReviewItem[]>(() => q.data.value ?? []);
  return reactive({
    items,
    loading: computed(() => q.isFetching.value),
    inboxError: computed(() => (q.error.value ? errorMessage(q.error.value) : null)),
    waitingCount: computed(() => items.value.filter((i) => !i.mine && i.myState === "new").length),
    reload: () => q.refetch(),
  });
}

/** Closed and merged changes (one page), read once for the Closed tab and its count. */
export function useClosedInbox(projectId: () => string) {
  const daemon = useDaemon();
  const q = useQuery({
    ...common,
    queryKey: computed(() => reviewKeys.closed(projectId())),
    queryFn: () => daemon.api.closedReviews(projectId()),
  });
  return reactive({
    closedItems: computed<ReviewItem[] | null>(() => q.data.value?.reviews ?? null),
    closedMore: computed(() => q.data.value?.more ?? false),
    closedLoading: computed(() => q.isFetching.value),
    closedError: computed(() => (q.error.value ? errorMessage(q.error.value) : null)),
  });
}

interface OpenedChange {
  detail: ReviewDetail;
  diffText: string;
  diff: ParsedDiff;
}

/**
 * Change `number`: its detail and diff, read together (when the diff's head differs from the
 * detail's, they are read once more so the two agree), and the reviewer's decision on it.
 */
export function useChange(projectId: () => string, number: () => number) {
  const daemon = useDaemon();
  const client = useQueryClient();

  async function read(recheck: boolean): Promise<OpenedChange> {
    const id = projectId();
    const n = number();
    // Lazy: the parser lives in `@pierre/diffs`, kept out of the app shell with the renderer.
    const [{ parseUnifiedDiff }, detail, { text, headSha }] = await Promise.all([
      import("../checkpoints/diff"),
      daemon.api.review(id, n),
      daemon.api.reviewDiff(id, n),
    ]);
    // The change moved between the two requests: the diff and the detail would disagree.
    if (recheck && headSha && headSha.toLowerCase() !== detail.headSha.toLowerCase()) return read(false);
    return { detail, diffText: text, diff: parseUnifiedDiff(text) };
  }

  const key = computed(() => reviewKeys.change(projectId(), number()));
  const q = useQuery({ ...common, queryKey: key, queryFn: () => read(true) });

  const decideError = ref<string | null>(null);
  const decision = useMutation({
    mutationFn: (v: { detail: ReviewDetail; decision: "greenlight" | "send_back"; comment?: string }) =>
      daemon.api.decideReview(projectId(), v.detail.number, {
        decision: v.decision,
        comment: v.comment,
        headSha: v.detail.headSha,
      }),
  });

  /**
   * Greenlight or send back the open change. The row and the change take the new state at once,
   * then the inbox, the change and its conversation are read again. `head_moved` reloads the
   * change and says why. Resolves true when it went through, or "head_moved" when the change moved.
   */
  async function decide(choice: "greenlight" | "send_back", comment?: string): Promise<boolean | "head_moved"> {
    const detail = q.data.value?.detail;
    if (!detail || decision.isPending.value) return false;
    const id = projectId();
    decideError.value = null;
    try {
      await decision.mutateAsync({ detail, decision: choice, comment });
      const myState = choice === "greenlight" ? "greenlit" : "sent_back";
      client.setQueryData<OpenedChange>(reviewKeys.change(id, detail.number), (old) =>
        old ? { ...old, detail: { ...old.detail, myState } } : old,
      );
      client.setQueryData<ReviewItem[]>(reviewKeys.inbox(id), (old) =>
        old?.map((i) => (i.number === detail.number ? { ...i, myState } : i)),
      );
      void client.invalidateQueries({ queryKey: reviewKeys.inbox(id) });
      void client.invalidateQueries({ queryKey: reviewKeys.change(id, detail.number) });
      return true;
    } catch (e) {
      const err = e as { status?: number; reason?: string };
      if (err.status === 409 && err.reason === "head_moved") {
        await client.invalidateQueries({ queryKey: reviewKeys.change(id, detail.number) });
        decideError.value = "This change was updated since you opened it";
        return "head_moved";
      }
      decideError.value = errorMessage(e);
      if (err.status === 404) void q.refetch();
      return false;
    }
  }

  const gone = computed(() => status(q.error.value) === 404);
  return reactive({
    current: computed(() => q.data.value?.detail ?? null),
    diffText: computed(() => q.data.value?.diffText ?? null),
    diff: computed(() => q.data.value?.diff ?? null),
    /** First load, with nothing to show yet. */
    opening: computed(() => q.isLoading.value),
    /** A read in place, with the change still on screen. */
    refreshing: computed(() => q.isFetching.value && !!q.data.value),
    gone,
    openError: computed(() => (q.error.value && !q.data.value ? errorMessage(q.error.value) : null)),
    refreshError: computed(() => (q.error.value && q.data.value && !gone.value ? errorMessage(q.error.value) : null)),
    refresh: () => q.refetch(),
    deciding: computed(() => decision.isPending.value),
    decideError,
    decide,
  });
}

/** What the Conversation tab counts: comments, and reviews that say something. */
const counted = (i: TimelineItem) => i.kind === "comment" || (i.kind === "review" && !!i.body);

/**
 * Change `number`'s conversation, read again when its head moves (keeping the old one on screen
 * meanwhile), and a way to comment on it.
 */
export function useConversation(projectId: () => string, number: () => number, headSha: () => string | undefined) {
  const daemon = useDaemon();
  const client = useQueryClient();
  const key = computed(() => reviewKeys.conversation(projectId(), number()));
  const q = useQuery({
    ...common,
    // The head is part of the key so a push reads it again; the change is what keys the cache.
    queryKey: computed(() => [...key.value, headSha() ?? ""] as const),
    queryFn: () => daemon.api.reviewConversation(projectId(), number()),
    enabled: computed(() => !!headSha()),
    // A moved head keeps showing the old conversation until the new one arrives; another change starts empty.
    placeholderData: (previous, query) =>
      query && query.queryKey[3] === number() && query.queryKey[1] === projectId() ? keepPreviousData(previous) : undefined,
  });

  const posting = useMutation({
    mutationFn: (body: string) => daemon.api.commentOnReview(projectId(), number(), body),
    onSuccess: (item) => {
      // GitHub's answer is the comment as stored: shown as is, without reading it all again.
      client.setQueryData<ReviewConversation>([...key.value, headSha() ?? ""], (old) =>
        old ? { ...old, items: [item, ...old.items] } : old,
      );
      // Its comment count changed.
      void client.invalidateQueries({ queryKey: reviewKeys.inbox(projectId()) });
    },
  });
  const postError = ref<string | null>(null);

  /** Posts `body`; true when it went through. */
  async function comment(body: string): Promise<boolean> {
    if (!body.trim() || posting.isPending.value) return false;
    postError.value = null;
    try {
      await posting.mutateAsync(body);
      return true;
    } catch (e) {
      postError.value = errorMessage(e);
      return false;
    }
  }

  const conversation = computed<ReviewConversation | null>(() => q.data.value ?? null);
  return {
    conversation,
    error: computed(() => (q.error.value ? errorMessage(q.error.value) : null)),
    loading: computed(() => q.isFetching.value),
    posting: computed(() => posting.isPending.value),
    postError,
    count: computed(() => conversation.value?.items.filter(counted).length ?? null),
    reload: () => q.refetch(),
    comment,
  };
}

/**
 * Change `number`'s deploy preview for its current head: checked every 10 s while the deploy
 * runs and the page is visible, and once more when a hidden page comes back.
 */
export function usePreview(projectId: () => string, number: () => number, headSha: () => string | undefined) {
  const daemon = useDaemon();
  const q = useQuery({
    ...common,
    queryKey: computed(() => reviewKeys.preview(projectId(), number(), headSha())),
    queryFn: () => daemon.api.reviewPreview(projectId(), number()),
    enabled: computed(() => !!headSha()),
    // A running deploy is always worth checking again, e.g. when a hidden page comes back.
    staleTime: (query) => (query.state.data?.state === "deploying" ? 0 : STALE_MS),
    refetchInterval: (query) => (query.state.data?.state === "deploying" ? DEPLOY_POLL_MS : false),
  });
  return {
    preview: computed<ReviewPreview | null>(() => q.data.value ?? null),
    error: computed(() => (q.error.value ? errorMessage(q.error.value) : null)),
    loading: computed(() => q.isFetching.value),
    reload: () => q.refetch(),
  };
}
