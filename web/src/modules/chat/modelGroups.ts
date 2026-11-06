/** A model as listed for an assistant CLI. */
export type ModelChoice = { id: string; name: string };

/** Display row: keep the full id for selection, show a short label. */
export type GroupedModel = ModelChoice & { label: string };

/** One section in the picker; `heading: null` is the ungrouped (Default / plain) block. */
export type ModelGroup = { heading: string | null; items: GroupedModel[] };

/** Split `provider/model` ids into nested headings; leave bare ids ungrouped. */
export function groupModels(models: ModelChoice[]): ModelGroup[] {
  const ungrouped: GroupedModel[] = [];
  const byPrefix = new Map<string, GroupedModel[]>();

  for (const m of models) {
    const slash = m.id.indexOf("/");
    if (slash <= 0 || slash === m.id.length - 1) {
      ungrouped.push({ ...m, label: m.name });
      continue;
    }
    const prefix = m.id.slice(0, slash);
    const rest = m.id.slice(slash + 1);
    const list = byPrefix.get(prefix) ?? [];
    list.push({ ...m, label: rest });
    byPrefix.set(prefix, list);
  }

  const groups: ModelGroup[] = [];
  if (ungrouped.length) groups.push({ heading: null, items: ungrouped });
  for (const [heading, items] of byPrefix) {
    groups.push({ heading, items });
  }
  return groups;
}
