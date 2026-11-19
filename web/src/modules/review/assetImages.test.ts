import { describe, expect, it } from "vitest";
import { repoOfPullUrl, rewriteAssetImages } from "./assetImages";

const to = (sha: string, path: string) => `/api/projects/site/reviews/assets?path=${encodeURIComponent(path)}&sha=${sha}`;

describe("rewriteAssetImages", () => {
  it("points the repo's review images at the daemon and leaves everything else", () => {
    const md = [
      "Look:",
      "![screenshot](https://github.com/Acme/Site/blob/abc1234/shots/7/2026-11-19/x.png?raw=true)",
      "![other](https://github.com/acme/site/blob/abc1234/README.png?raw=true)",
      "![elsewhere](https://github.com/evil/site/blob/abc1234/shots/7/2026-11-19/x.png?raw=true)",
      "[link](https://github.com/acme/site/blob/abc1234/uploads/7/2026-11-19/y.png?raw=true)",
      "![upload](https://github.com/acme/site/blob/def5678/uploads/7/2026-11-19/y.webp?raw=true)",
    ].join("\n");
    const out = rewriteAssetImages(md, "acme/site", to).split("\n");
    expect(out[1]).toBe(`![screenshot](${to("abc1234", "shots/7/2026-11-19/x.png")})`);
    // Not in the assets folders, another repo, or a link rather than an image: untouched.
    expect(out[2]).toContain("https://github.com/acme/site/blob/abc1234/README.png");
    expect(out[3]).toContain("https://github.com/evil/site/");
    expect(out[4]).toContain("https://github.com/acme/site/");
    expect(out[5]).toBe(`![upload](${to("def5678", "uploads/7/2026-11-19/y.webp")})`);
  });
});

describe("repoOfPullUrl", () => {
  it("reads owner/name from a pull request URL", () => {
    expect(repoOfPullUrl("https://github.com/acme/site/pull/7")).toBe("acme/site");
    expect(repoOfPullUrl("https://example.com/x")).toBeNull();
    expect(repoOfPullUrl(null)).toBeNull();
  });
});
