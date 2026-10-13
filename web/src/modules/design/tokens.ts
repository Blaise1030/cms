/** Helpers for a token group (`colors`, `rounded`, …): plain objects whose key order is meaningful. */
type Group = Record<string, unknown>;

/** Renames a token in place, keeping its position; `null` when the new name is empty or taken. */
export function renameKey(group: Group, from: string, to: string): Group | null {
  if (to === "" || (to !== from && Object.hasOwn(group, to))) return null;
  return Object.fromEntries(Object.entries(group).map(([key, value]) => [key === from ? to : key, value]));
}

export function removeKey(group: Group, key: string): Group {
  const { [key]: _removed, ...rest } = group;
  return rest;
}

/** Appends a token named `base`, or `base-2`, `base-3`, … when that name is taken. */
export function addKey(group: Group, base: string, value: unknown): Group {
  let name = base;
  for (let n = 2; Object.hasOwn(group, name); n++) name = `${base}-${n}`;
  return { ...group, [name]: value };
}
