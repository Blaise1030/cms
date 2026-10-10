import { describe, expect, it } from "vitest";
import { parseUnifiedDiff } from "./diff";

const SAMPLE = `commit abc123
Author: a <a@b.c>

    checkpoint: hero copy

diff --git a/src/hero.mdx b/src/hero.mdx
index 111..222 100644
--- a/src/hero.mdx
+++ b/src/hero.mdx
@@ -1,3 +1,3 @@
 title: Hello
-body: old
+body: new
 cta: go
@@ -10,2 +10,3 @@ tail
 keep
+added
 end
diff --git a/new.txt b/new.txt
new file mode 100644
--- /dev/null
+++ b/new.txt
@@ -0,0 +1,2 @@
+one
+two
diff --git a/logo.png b/logo.png
Binary files a/logo.png and b/logo.png differ
`;

describe("parseUnifiedDiff", () => {
  const parsed = parseUnifiedDiff(SAMPLE);

  it("keeps commit metadata as header", () => {
    expect(parsed.header).toContain("checkpoint: hero copy");
    expect(parsed.files.map((f) => f.path)).toEqual(["src/hero.mdx", "new.txt", "logo.png"]);
  });

  it("counts changed lines across hunks", () => {
    const f = parsed.files[0]!;
    expect(f).toMatchObject({ added: 2, removed: 1, status: "modified", binary: false });
    expect(f.meta.hunks).toHaveLength(2);
  });

  it("handles new files and binary files", () => {
    expect(parsed.files[1]).toMatchObject({ added: 2, removed: 0, status: "added" });
    expect(parsed.files[2]!.binary).toBe(true);
  });

  it("does not mistake '--- ' removed lines inside a hunk for headers", () => {
    const d = parseUnifiedDiff("diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,1 +1,1 @@\n--- comment\n+++ comment\n");
    expect(d.files[0]).toMatchObject({ path: "x", added: 1, removed: 1 });
  });

  it("returns no files for empty input", () => {
    expect(parseUnifiedDiff("")).toEqual({ header: "", files: [] });
  });
});
