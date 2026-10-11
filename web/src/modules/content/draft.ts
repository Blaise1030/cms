import type { ContentEntry, FieldDescriptor } from "@/core/types";

export interface DraftChange {
  changed: boolean;
  fields: Record<string, unknown>;
  unset: string[];
  body?: string;
}

export function cloneValue<T>(value: T): T {
  return value === undefined ? value : (JSON.parse(JSON.stringify(value)) as T);
}

export function sameValue(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/**
 * The minimal update that turns the loaded entry into the draft: changed top-level fields,
 * top-level optional fields the writer cleared, and the body when it differs.
 * Without a descriptor the entry is read-only and never has pending changes.
 */
export function diffDraft(
  entry: ContentEntry,
  fields: Record<string, unknown>,
  body: string,
  descriptor: FieldDescriptor | null,
): DraftChange {
  const result: DraftChange = { changed: false, fields: {}, unset: [] };
  if (!descriptor) return result;
  const loaded = entry.fields ?? {};
  for (const key of new Set([...Object.keys(loaded), ...Object.keys(fields)])) {
    const before = loaded[key], after = fields[key];
    if (sameValue(before, after)) continue;
    if (after === undefined) {
      // A cleared required field is sent as null so the daemon reports it against its path.
      if (descriptor.properties?.[key]?.required === false) result.unset.push(key);
      else result.fields[key] = null;
    } else result.fields[key] = after;
  }
  if (entry.kind !== "json" && body !== (entry.body ?? "")) result.body = body;
  result.changed = !!Object.keys(result.fields).length || !!result.unset.length || result.body !== undefined;
  return result;
}
