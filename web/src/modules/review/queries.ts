import { computed, reactive, ref } from "vue";
import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/vue-query";
import { useDaemon } from "@/core/daemon";
import type { ReviewConversation, ReviewDetail, ReviewItem, ReviewPreview, TimelineItem } from "@/core/types";
import { useGithubStore } from "@/modules/github/store";
import type { ShotItem } from "./shots/queue";
import { shotItemsForReview } from "./shots/send";
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
    /** The `orso.json` `prLabels` the inbox filters on, as the daemon read them. */
    labels: computed<string[]>(() => q.data.value?.labels ?? []),
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
    mutationFn: (v: { detail: ReviewDetail; decision: "greenlight" | "send_back"; comment?: string; shots?: ShotItem[] }) =>
      v.shots?.length
        ? // Queued screenshot comments go in the same review.
          daemon.api.reviewWithShots(projectId(), v.detail.number, {
            event: v.decision === "greenlight" ? "APPROVE" : "REQUEST_CHANGES",
            body: v.comment ?? "",
            headSha: v.detail.headSha,
            items: shotItemsForReview(v.shots),
          })
        : daemon.api.decideReview(projectId(), v.detail.number, {
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
  async function decide(
    choice: "greenlight" | "send_back",
    comment?: string,
    shots?: ShotItem[],
  ): Promise<boolean | "head_moved"> {
    const detail = q.data.value?.detail;
    if (!detail || decision.isPending.value) return false;
    const id = projectId();
    decideError.value = null;
    try {
      await decision.mutateAsync({ detail, decision: choice, comment, shots });
      // The screenshots are in the review now; the conversation shows it.
      if (shots?.length) void client.invalidateQueries({ queryKey: reviewKeys.conversation(id, detail.number) });
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

  /**
   * Comments being sent, or that failed, shown after the server's items until they resolve.
   * They live outside the query cache, so a background read of the conversation can't drop them.
   */
  const pending = ref<PendingComment[]>([]);
  /** Server comment id → the client id its pending row had, so the row keeps its key (no remount). */
  const clientKeys = reactive(new Map<number, string>());
  const github = useGithubStore();
  if (!github.user) void github.load().catch(() => undefined);
  let seq = 0;

  const setPending = (clientId: string, change: Partial<PendingComment> | null) => {
    pending.value = change
      ? pending.value.map((p) => (p.clientId === clientId ? { ...p, ...change } : p))
      : pending.value.filter((p) => p.clientId !== clientId);
  };

  const posting = useMutation({
    // One scope per change: quick sends reach GitHub (and the feed) in the order they were sent.
    scope: { id: `review-comment:${projectId()}:${number()}` },
    mutationFn: (v: SendComment) =>
      daemon.api.commentOnReview(
        projectId(),
        number(),
        v.body,
        v.images.map((i) => ({ base64: i.dataUrl.slice(i.dataUrl.indexOf(",") + 1), mime: i.mime, name: i.name })),
      ),
    onMutate: (v) => {
      if (pending.value.some((p) => p.clientId === v.clientId))
        setPending(v.clientId, { state: "sending", images: v.images, imagesRefused: false });
      else
        pending.value = [
          ...pending.value,
          { clientId: v.clientId, body: v.body, images: v.images, at: new Date().toISOString(), state: "sending" },
        ];
    },
    onSuccess: async (item, v) => {
      const queryKey = [...key.value, headSha() ?? ""];
      // A read already in flight would put the list back without this comment.
      await client.cancelQueries({ queryKey: key.value });
      if (item.kind === "comment") clientKeys.set(item.id, v.clientId);
      // GitHub's answer replaces the pending row in place: appended to the server's items
      // (oldest first) as the pending row leaves, in one update.
      client.setQueryData<ReviewConversation>(queryKey, (old) =>
        old && !old.items.some((i) => i.kind === "comment" && item.kind === "comment" && i.id === item.id)
          ? { ...old, items: [...old.items, item] }
          : old,
      );
      setPending(v.clientId, null);
      // Its comment count changed.
      void client.invalidateQueries({ queryKey: reviewKeys.inbox(projectId()) });
    },
    onError: (e, v) => {
      const err = e as { status?: number; reason?: string };
      setPending(v.clientId, { state: "failed", imagesRefused: err.reason === "assets_refused" });
    },
  });

  /** Sends `body` (and `images`) at once, optimistically; false (nothing sent) when both are empty. */
  function comment(body: string, images: AttachedImage[] = []): boolean {
    if (!body.trim() && images.length === 0) return false;
    posting.mutate({ clientId: `pending-${Date.now()}-${++seq}`, body, images });
    return true;
  }
  /** Sends a failed message again, images and all, in the same place. */
  function retry(clientId: string) {
    const failed = pending.value.find((p) => p.clientId === clientId && p.state === "failed");
    if (failed) posting.mutate({ clientId, body: failed.body, images: failed.images ?? [] });
  }
  /** Sends a failed message's text alone (its images couldn't be stored), in the same place. */
  function sendWithoutImages(clientId: string) {
    const failed = pending.value.find((p) => p.clientId === clientId && p.state === "failed");
    if (failed?.body.trim()) posting.mutate({ clientId, body: failed.body, images: [] });
  }
  /** Drops a failed message; nothing goes back to the composer. */
  const discard = (clientId: string) => setPending(clientId, null);

  const conversation = computed<ReviewConversation | null>(() => {
    const data = q.data.value;
    if (!data) return null;
    const me = github.user;
    const items: TimelineItem[] = data.items.map((i) =>
      i.kind === "comment" && clientKeys.has(i.id) ? { ...i, clientId: clientKeys.get(i.id) } : i,
    );
    for (const p of pending.value) {
      items.push({
        kind: "comment",
        id: -1,
        author: me?.login ?? "You",
        avatarUrl: me?.avatarUrl,
        at: p.at,
        body: p.body,
        url: "",
        clientId: p.clientId,
        pending: p.state,
        images: p.images?.map((i) => i.dataUrl),
        imagesRefused: p.imagesRefused,
      });
    }
    return { ...data, items };
  });
  return {
    conversation,
    error: computed(() => (q.error.value ? errorMessage(q.error.value) : null)),
    loading: computed(() => q.isFetching.value),
    count: computed(() => conversation.value?.items.filter(counted).length ?? null),
    reload: () => q.refetch(),
    comment,
    retry,
    sendWithoutImages,
    /** A message's images as they were attached, for copying when GitHub couldn't store them. */
    imagesOf: (clientId: string) => pending.value.find((p) => p.clientId === clientId)?.images ?? [],
    discard,
  };
}

/** An image attached in the composer. */
export interface AttachedImage {
  name: string;
  mime: string;
  /** `data:{mime};base64,…` */
  dataUrl: string;
}

interface SendComment {
  clientId: string;
  body: string;
  images: AttachedImage[];
}

/** A comment on its way to GitHub, or one that didn't get there. */
interface PendingComment {
  clientId: string;
  body: string;
  images?: AttachedImage[];
  at: string;
  state: "sending" | "failed";
  /** The repository refused to store its images. */
  imagesRefused?: boolean;
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
