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

/** Move `fromId` in front of `toId` (drop-on-tab reorder). */
export function moveSessionId(ids: string[], fromId: string, toId: string): string[] {
  if (fromId === toId) return ids;
  const from = ids.indexOf(fromId);
  if (from < 0 || !ids.includes(toId)) return ids;
  const next = ids.slice();
  next.splice(from, 1);
  const insertAt = next.indexOf(toId);
  if (insertAt < 0) return ids;
  next.splice(insertAt, 0, fromId);
  return next;
}
