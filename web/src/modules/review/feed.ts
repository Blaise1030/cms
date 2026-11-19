import type { TimelineItem } from "@/core/types";

/** Messages by one author this close together share one header. */
export const GROUP_MS = 5 * 60_000;

type Comment = Extract<TimelineItem, { kind: "comment" }>;
type Review = Extract<TimelineItem, { kind: "review" }>;

/** Something someone wrote: a comment, or a review that only comments. */
export interface FeedMessage {
  type: "message";
  key: string;
  author: string;
  avatarUrl?: string;
  at: string;
  body: string;
  url?: string;
  /** Follows a message by the same author within 5 minutes: no avatar or name of its own. */
  grouped: boolean;
  /** Still being sent, or it didn't get there (the client id is then in `key`). */
  pending?: "sending" | "failed";
  /** Images it is sending (data URLs). */
  images?: string[];
  /** It failed because the repository refused to store its images. */
  imagesRefused?: boolean;
}

/** Something that happened: a greenlight or send-back (with its comment), a push, a merge or close. */
export interface FeedEvent {
  type: "event";
  key: string;
  item: Exclude<TimelineItem, Comment> | Review;
}

export interface FeedDay {
  type: "day";
  key: string;
  label: string;
}

export type FeedRow = FeedMessage | FeedEvent | FeedDay;

const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();

/** "Today", "Yesterday", else the date (with the year when it isn't this year's). */
export function dayLabel(at: Date, now = new Date()): string {
  const days = Math.round((startOfDay(now) - startOfDay(at)) / 86_400_000);
  if (days === 0) return "Today";
  if (days === 1) return "Yesterday";
  return at.toLocaleDateString(undefined, {
    weekday: "long",
    month: "long",
    day: "numeric",
    ...(at.getFullYear() === now.getFullYear() ? {} : { year: "numeric" }),
  });
}

/** The conversation, oldest first, as rows: a divider before each new day, messages grouped. */
export function buildFeed(items: TimelineItem[], now = new Date()): FeedRow[] {
  const rows: FeedRow[] = [];
  let day: number | null = null;
  let last: FeedMessage | null = null;
  items.forEach((item, i) => {
    const at = new Date(item.at);
    const valid = !Number.isNaN(at.getTime());
    if (valid && startOfDay(at) !== day) {
      day = startOfDay(at);
      rows.push({ type: "day", key: `day-${day}`, label: dayLabel(at, now) });
      last = null;
    }
    const message =
      item.kind === "comment" || (item.kind === "review" && item.verdict === "commented" && item.body)
        ? item
        : null;
    if (!message) {
      rows.push({ type: "event", key: `event-${i}`, item: item as FeedEvent["item"] });
      last = null;
      return;
    }
    const prev: FeedMessage | null = last;
    const grouped =
      !!prev && prev.author === message.author && valid && at.getTime() - new Date(prev.at).getTime() <= GROUP_MS;
    const row: FeedMessage = {
      type: "message",
      // A sent comment keeps its pending row's key, so it doesn't remount when it arrives.
      key: message.kind === "comment" ? (message.clientId ?? `comment-${message.id}`) : `review-${i}`,
      author: message.author,
      avatarUrl: message.avatarUrl,
      at: message.at,
      body: message.body ?? "",
      url: message.url || undefined,
      grouped,
      pending: message.kind === "comment" ? message.pending : undefined,
      images: message.kind === "comment" ? message.images : undefined,
      imagesRefused: message.kind === "comment" ? message.imagesRefused : undefined,
    };
    rows.push(row);
    last = row;
  });
  return rows;
}

/** "3:42 PM", in the viewer's locale. */
export const clockTime = (at: string) =>
  new Date(at).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });

/** The full date and time, for a tooltip. */
export const exactTime = (at: string) =>
  new Date(at).toLocaleString(undefined, { dateStyle: "full", timeStyle: "short" });
