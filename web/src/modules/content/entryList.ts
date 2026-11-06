import type { Component } from "vue";
import { Copy, FileText, Link, Trash2 } from "@lucide/vue";
import type { ContentEntrySummary } from "@/core/types";
import { humanize } from "./labels";

/** One row of an entry's menu; `"separator"` draws a divider. */
export type EntryAction = "separator" | {
  id: string;
  label: string;
  icon?: Component;
  destructive?: boolean;
  disabled?: boolean;
  /** Why the action is disabled, shown under it. */
  hint?: string | null;
  run: () => void;
};

/** What each menu item does; the panel owns the copy and the lifecycle calls. */
export interface EntryActionHandlers {
  open: () => void;
  copyLink: () => void;
  copyPath: () => void;
  remove: () => void;
}

/** Why a delete is refused while the open entry has unsaved edits. */
const PENDING = "Save or discard your changes first";

/** Entries whose slug contains `query`, ignoring case; an empty query keeps every row. */
export function filterEntries(entries: readonly ContentEntrySummary[], query: string): ContentEntrySummary[] {
  const needle = query.trim().toLowerCase();
  return needle ? entries.filter(e => e.id.toLowerCase().includes(needle)) : entries.slice();
}

/** The same entries, ordered by the name a writer reads; the list it was given is left alone. */
export function sortEntries(entries: readonly ContentEntrySummary[]): ContentEntrySummary[] {
  return entries
    .slice()
    .sort((a, b) => humanize(a.id).localeCompare(humanize(b.id), undefined, { sensitivity: "base" }));
}

/** The menu for one entry row: open and share always, path when known, delete unless the collection is a singleton. */
export function entryActions(ctx: {
  singleton: boolean;
  path?: string;
  dirty: boolean;
  handlers: EntryActionHandlers;
}): EntryAction[] {
  const { singleton, path, dirty, handlers } = ctx;
  const blocked = dirty ? PENDING : null;
  const actions: EntryAction[] = [
    { id: "open", label: "Open", icon: FileText, run: handlers.open },
    "separator",
    { id: "copy-link", label: "Copy link", icon: Link, run: handlers.copyLink },
  ];
  if (path) actions.push({ id: "copy-path", label: "Copy file path", icon: Copy, run: handlers.copyPath });
  if (!singleton) actions.push(
    "separator",
    { id: "delete", label: "Delete", icon: Trash2, destructive: true, disabled: dirty, hint: blocked, run: handlers.remove },
  );
  return actions;
}
