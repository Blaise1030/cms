// Turn a unified diff (`git show` / `git diff` output) into per-file before/after text so
// `@codemirror/merge` can render it. Only hunks are available, so each side is the hunk lines
// joined by an identical `@@` marker line (which the merge view treats as unchanged context).

export interface FileDiff {
  path: string;
  oldText: string;
  newText: string;
  added: number;
  removed: number;
  binary: boolean;
}

export interface ParsedDiff {
  /** Anything before the first `diff --git` (commit metadata, when present). */
  header: string;
  files: FileDiff[];
}

interface Building {
  path: string;
  old: string[];
  new: string[];
  added: number;
  removed: number;
  binary: boolean;
  inHunk: boolean;
}

function finish(b: Building): FileDiff {
  return {
    path: b.path,
    oldText: b.old.join("\n"),
    newText: b.new.join("\n"),
    added: b.added,
    removed: b.removed,
    binary: b.binary,
  };
}

function stripPrefix(p: string): string {
  return p.replace(/^[ab]\//, "");
}

export function parseUnifiedDiff(text: string): ParsedDiff {
  const files: FileDiff[] = [];
  const header: string[] = [];
  let cur: Building | null = null;

  for (const line of text.replace(/\r\n/g, "\n").split("\n")) {
    const git = /^diff --git a\/(.*) b\/(.*)$/.exec(line);
    if (git) {
      if (cur) files.push(finish(cur));
      cur = {
        path: git[2]!,
        old: [],
        new: [],
        added: 0,
        removed: 0,
        binary: false,
        inHunk: false,
      };
      continue;
    }
    if (!cur) {
      header.push(line);
      continue;
    }
    if (line.startsWith("@@")) {
      cur.inHunk = true;
      cur.old.push(line);
      cur.new.push(line);
      continue;
    }
    if (!cur.inHunk) {
      if (line.startsWith("Binary files") || line.startsWith("GIT binary patch")) cur.binary = true;
      else if (line.startsWith("+++ ") && !line.includes("/dev/null")) cur.path = stripPrefix(line.slice(4));
      else if (line.startsWith("--- ") && !line.includes("/dev/null")) cur.path = stripPrefix(line.slice(4));
      continue;
    }
    if (line.startsWith("\\")) continue; // "\ No newline at end of file"
    const marker = line[0];
    const body = line.slice(1);
    if (marker === "+") {
      cur.new.push(body);
      cur.added++;
    } else if (marker === "-") {
      cur.old.push(body);
      cur.removed++;
    } else if (marker === " " || line === "") {
      cur.old.push(body);
      cur.new.push(body);
    }
  }
  if (cur) files.push(finish(cur));

  return { header: header.join("\n").trim(), files };
}
