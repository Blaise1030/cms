import type { ProjectTab } from "@/router";

/** Focused fields the current project page exposes (route params, query, store). */
export type PageContextParams = Record<string, string | undefined | null>;

export type PageContext = {
  tab: ProjectTab;
  fields: Record<string, string>;
};

type PageContextProvider = {
  /** Pick the focused fields for this tab from whatever the page stored. */
  fields: (params: PageContextParams) => Record<string, string>;
  /** Short suffix after the tab name in the composer/transcript badge. */
  badge: (fields: Record<string, string>) => string;
};

const TAB_LABEL: Record<ProjectTab, string> = {
  preview: "Preview",
  content: "Content",
  design: "Design",
  code: "Code",
  history: "History",
  settings: "Settings",
};

/**
 * Per-tab readers. Add a tab here to inject its page params into chat —
 * typically whatever that page already keeps in the route/query/store.
 */
export const PAGE_CONTEXT_PROVIDERS: Partial<Record<ProjectTab, PageContextProvider>> = {
  preview: {
    fields: (params) => ({ path: params.path?.trim() || "/" }),
    badge: (fields) => fields.path ?? "/",
  },
  content: {
    fields: (params) => {
      const fields: Record<string, string> = {};
      if (params.collection) fields.collection = params.collection;
      if (params.entry) fields.entry = params.entry;
      return fields;
    },
    badge: (fields) => [fields.collection, fields.entry].filter(Boolean).join("/"),
  },
};

const BLOCK = /\[page-context\]\n([\s\S]*?)\n\[\/page-context\](?:\n|$)/;

/** Resolve ambient page context for a tab, or null when that tab has no provider. */
export function resolvePageContext(input: {
  tab: ProjectTab;
  params: PageContextParams;
}): PageContext | null {
  const provider = PAGE_CONTEXT_PROVIDERS[input.tab];
  if (!provider) return null;
  return { tab: input.tab, fields: provider.fields(input.params) };
}

/** Wire block prepended to the outbound chat text. */
export function formatPageContext(ctx: PageContext): string {
  const lines = [`tab: ${ctx.tab}`, ...Object.entries(ctx.fields).map(([k, v]) => `${k}: ${v}`)];
  return `[page-context]\n${lines.join("\n")}\n[/page-context]`;
}

/** Composer / transcript chip label. */
export function pageContextBadge(ctx: PageContext): string {
  const provider = PAGE_CONTEXT_PROVIDERS[ctx.tab];
  const detail = provider?.badge(ctx.fields) ?? "";
  const tab = TAB_LABEL[ctx.tab];
  return detail ? `${tab} · ${detail}` : tab;
}

/** Undo [withPageContext] for display. */
export function splitPageContext(text: string): { text: string; context: PageContext | null } {
  const match = BLOCK.exec(text);
  if (!match) return { text, context: null };
  const body = match[1] ?? "";
  const fields: Record<string, string> = {};
  let tab: ProjectTab | null = null;
  for (const line of body.split("\n")) {
    const i = line.indexOf(": ");
    if (i < 0) continue;
    const key = line.slice(0, i);
    const value = line.slice(i + 2);
    if (key === "tab") tab = value as ProjectTab;
    else fields[key] = value;
  }
  const context = tab ? { tab, fields } : null;
  let before = text.slice(0, match.index);
  let after = text.slice(match.index + match[0].length);
  // Drop the single newline that joined the block when it was prepended or appended.
  if (before.endsWith("\n") && after.startsWith("\n")) after = after.slice(1);
  else if (before.endsWith("\n") && !after) before = before.slice(0, -1);
  else if (!before && after.startsWith("\n")) after = after.slice(1);
  return { text: before + after, context };
}

/** Append page context for the agent; replaces any existing block. */
export function withPageContext(text: string, ctx: PageContext | null): string {
  const base = splitPageContext(text).text;
  if (!ctx) return base;
  return base ? `${base}\n${formatPageContext(ctx)}` : formatPageContext(ctx);
}

/** Composer sticky state: live on Preview/Content, frozen after leaving those tabs. */
export type StickyPageContext = {
  context: PageContext | null;
  /** Holding a prior Preview/Content context while on another module. */
  frozen: boolean;
};

/** Tabs that refresh the sticky badge from the current page. */
export function isLivePageContextTab(tab: ProjectTab): boolean {
  return PAGE_CONTEXT_PROVIDERS[tab] != null;
}

/** Advance sticky state when the route/tab or focused page changes. */
export function nextStickyPageContext(
  prev: StickyPageContext,
  currentTab: ProjectTab,
  resolved: PageContext | null,
): StickyPageContext {
  if (isLivePageContextTab(currentTab) && resolved) {
    return { context: resolved, frozen: false };
  }
  if (prev.context) return { context: prev.context, frozen: true };
  return { context: null, frozen: false };
}

/** Drop sticky context so the next send injects nothing until back on a live tab. */
export function clearStickyPageContext(): StickyPageContext {
  return { context: null, frozen: false };
}
