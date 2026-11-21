import { describe, expect, it } from "vitest";
import { localizePreviewAssetUrl, previewMediaSrc, previewSitePath } from "./previewAsset";

const page = "http://orso.localhost:51995/blog/";

describe("previewSitePath", () => {
  it("keeps only the path of absolute and relative hrefs", () => {
    expect(previewSitePath("/favicon.svg", page)).toBe("/favicon.svg");
    expect(previewSitePath("https://astro-marketing.blaisetiong-dev.workers.dev/og.png", page)).toBe("/og.png");
    expect(previewSitePath("https://example.com/blog/welcome/?x=1", page)).toBe("/blog/welcome/?x=1");
    expect(previewSitePath("cover.png", page)).toBe("/blog/cover.png");
  });

  it("is empty for a missing href", () => {
    expect(previewSitePath("", page)).toBe("");
  });
});

describe("localizePreviewAssetUrl", () => {
  it("keeps assets already on the preview origin", () => {
    expect(localizePreviewAssetUrl("http://orso.localhost:51995/favicon.svg", page)).toBe(
      "http://orso.localhost:51995/favicon.svg",
    );
  });

  it("maps a published absolute URL onto the preview origin by path", () => {
    expect(localizePreviewAssetUrl("https://example.com/og.png?v=2", page)).toBe(
      "http://orso.localhost:51995/og.png?v=2",
    );
  });

  it("serves site-root and relative paths from the preview origin", () => {
    expect(localizePreviewAssetUrl("/og.png", page)).toBe("http://orso.localhost:51995/og.png");
    expect(localizePreviewAssetUrl("cover.png", page)).toBe("http://orso.localhost:51995/blog/cover.png");
  });

  it("leaves data URLs and empty hrefs alone", () => {
    expect(localizePreviewAssetUrl("data:image/png;base64,AAAA", page)).toBe("data:image/png;base64,AAAA");
    expect(localizePreviewAssetUrl("", page)).toBe("");
  });
});

describe("previewMediaSrc", () => {
  it("loads the asset from the preview origin, without any token in the URL", () => {
    expect(previewMediaSrc("https://cdn.example/og.png", page)).toBe("http://orso.localhost:51995/og.png");
  });
});
