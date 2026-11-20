import { readPref, writePref } from "@orso/utils";

export const SESSION_ORDER_KEY = (projectId: string) => `orso.chat.sessionOrder.${projectId}`;

/** Saved session-tab order for a project (local preference). */
export function readSessionOrder(projectId: string): string[] {
  const raw = readPref(SESSION_ORDER_KEY(projectId));
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((id): id is string => typeof id === "string");
  } catch {
    return [];
  }
}

export function writeSessionOrder(projectId: string, ids: string[]) {
  writePref(SESSION_ORDER_KEY(projectId), ids.length ? JSON.stringify(ids) : null);
}

/** Reorder `ids` to match `order`, appending any ids that aren't listed yet. */
export function applySessionOrder(ids: string[], order: string[]): string[] {
  const remaining = new Set(ids);
  const sorted: string[] = [];
  for (const id of order) {
    if (!remaining.has(id)) continue;
    sorted.push(id);
    remaining.delete(id);
  }
  for (const id of ids) {
    if (remaining.has(id)) sorted.push(id);
  }
  return sorted;
}

/**
 * Move `fromId` into `toId`'s place (drag-over reorder). Dragging right lands after the
 * target and dragging left before it, so every position, the last included, is reachable.
 */
export function moveSessionId(ids: string[], fromId: string, toId: string): string[] {
  const to = ids.indexOf(toId);
  return to < 0 ? ids : moveSessionToIndex(ids, fromId, to);
}

/** Move `id` to index `to` (clamped); keyboard reorder and drag-over both land here. */
export function moveSessionToIndex(ids: string[], id: string, to: number): string[] {
  const from = ids.indexOf(id);
  const target = Math.max(0, Math.min(to, ids.length - 1));
  if (from < 0 || from === target) return ids;
  const next = ids.slice();
  next.splice(from, 1);
  next.splice(target, 0, id);
  return next;
}
