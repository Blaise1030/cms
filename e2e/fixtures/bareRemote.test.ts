import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import { createBareRemote, DEFAULT_REPO } from "./bareRemote";

const tmpDirs: string[] = [];

afterEach(() => {
  for (const d of tmpDirs.splice(0)) {
    fs.rmSync(d, { recursive: true, force: true });
  }
});

describe("bareRemote", () => {
  it("creates a bare remote with a seeded main branch", () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "bare-remote-"));
    tmpDirs.push(root);
    const bare = createBareRemote(root, DEFAULT_REPO);
    expect(fs.existsSync(bare)).toBe(true);
    const head = execFileSync("git", ["-C", bare, "rev-parse", "main"], { encoding: "utf8" }).trim();
    expect(head).toMatch(/^[0-9a-f]{40}$/);
    const tree = execFileSync("git", ["-C", bare, "ls-tree", "-r", "main", "--name-only"], {
      encoding: "utf8",
    });
    expect(tree).toContain("package.json");
  });
});
