// Turn a unified diff (`git show` / `git diff` output) into per-file diffs for `@pierre/diffs`
// to render, plus the change counts and git status the panel and file tree show.

import { parsePatchFiles, type FileDiffMetadata } from "@pierre/diffs";
import type { GitStatus } from "@pierre/trees";

export interface FileDiff {
  path: string;
  added: number;
  removed: number;
  /** No hunks: a binary file, or a pure rename / mode change. */
  binary: boolean;
  status: GitStatus;
  meta: FileDiffMetadata;
}

export interface ParsedDiff {
  /** Anything before the first `diff --git` (commit metadata, when present). */
  header: string;
  files: FileDiff[];
}

const STATUS: Record<FileDiffMetadata["type"], GitStatus> = {
  change: "modified",
  new: "added",
  deleted: "deleted",
  "rename-pure": "renamed",
  "rename-changed": "renamed",
};

function toFile(meta: FileDiffMetadata): FileDiff {
  let added = 0;
  let removed = 0;
  for (const hunk of meta.hunks) {
    for (const part of hunk.hunkContent) {
      if (part.type === "change") {
        added += part.additions;
        removed += part.deletions;
      }
    }
  }
  return {
    path: meta.name,
    added,
    removed,
    binary: meta.hunks.length === 0 && meta.type !== "rename-pure",
    status: STATUS[meta.type],
    meta,
  };
}

export function parseUnifiedDiff(text: string): ParsedDiff {
  const patches = parsePatchFiles(text.replace(/\r\n/g, "\n"));
  return {
    header: patches.map((p) => p.patchMetadata ?? "").join("\n").trim(),
    files: patches.flatMap((p) => p.files.map(toFile)),
  };
}
