import { parseDiffFromFile, type FileDiffMetadata } from "@pierre/diffs";
import type { FileVersion } from "@/core/types";

export type VersionBadge = "Current" | "Published" | "Draft";

/** The newest version is the working copy's base; older ones are tagged by whether they reached the remote. */
export function versionBadge(index: number, v: FileVersion): VersionBadge {
  if (index === 0) return "Current";
  return v.published ? "Published" : "Draft";
}

const clock = new Intl.DateTimeFormat("en-US", { hour: "numeric", minute: "2-digit" });
const date = new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric" });

export function versionTime(ts: number, now: Date = new Date()): string {
  const at = new Date(ts * 1000);
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  const day =
    at.toDateString() === now.toDateString() ? "Today"
    : at.toDateString() === yesterday.toDateString() ? "Yesterday"
    : date.format(at);
  return `${day} at ${clock.format(at)}`;
}

export function versionDiff(path: string, oldSource: string, newSource: string): FileDiffMetadata {
  return parseDiffFromFile({ name: path, contents: oldSource }, { name: path, contents: newSource });
}
