// Small presentational helpers for the assistant setup rows. No Vue and no
// daemon calls, so the copy and formatting are easy to unit test.
//
// Copy rules (plan §5): never show "agent", "CLI", "ACP", "SDK", "npx", "PATH"
// or "runtime". Keep the product words: assistant, download, sign in, version.

import type { RuntimeSource } from "@/core/types";
import type { RowTone } from "./rows";

/** One decimal below 10, whole numbers above: "9.6 MB", "150 MB". */
function round(n: number): string {
  return n >= 10 ? String(Math.round(n)) : String(Math.round(n * 10) / 10);
}

/**
 * "About 150 MB" for a download size. Returns null when the daemon does not
 * report a size, so the row can leave the line out instead of showing "0 MB".
 */
export function formatDownloadSize(bytes: number | null | undefined): string | null {
  if (bytes == null || !Number.isFinite(bytes) || bytes <= 0) return null;
  const kb = bytes / 1024;
  if (kb < 1024) return `About ${Math.max(1, Math.round(kb))} KB`;
  const mb = kb / 1024;
  if (mb < 1024) return `About ${round(mb)} MB`;
  return `About ${round(mb / 1024)} GB`;
}

/** Where a ready copy came from, in plain language. */
export function sourceLabel(source: RuntimeSource | null | undefined): string | null {
  if (source === "managed") return "Downloaded by orso";
  if (source === "system") return "Already on your Mac";
  return null;
}

export interface ToneBadge {
  variant: "secondary" | "outline" | "destructive";
  class: string;
}

/**
 * The badge look for each row tone. The shadcn badges have no success/warning
 * variant, so a tint stands in for them while errors use `destructive`.
 * `muted` rows (not downloaded, downloading, setting up) render no badge.
 */
export function toneBadge(tone: RowTone): ToneBadge | null {
  switch (tone) {
    case "success":
      return { variant: "secondary", class: "text-emerald-700 dark:text-emerald-400" };
    case "warning":
      return { variant: "outline", class: "border-amber-600/40 text-amber-700 dark:text-amber-400" };
    case "destructive":
      return { variant: "destructive", class: "" };
    default:
      return null;
  }
}
