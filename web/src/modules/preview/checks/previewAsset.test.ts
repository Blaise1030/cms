import { describe, expect, it } from "vitest";
import { localizePreviewAssetUrl, previewMediaSrc, previewSitePath } from "./previewAsset";

describe("previewSitePath", () => {
  const page = "http://127.0.0.1:51995/preview/astro-marketing/blog/";

  it("strips the preview mount from absolute and relative hrefs", () => {
    expect(previewSitePath("/preview/astro-marketing/favicon.svg", page)).toBe("/favicon.svg");
    expect(
      previewSitePath(
        "https://astro-marketing.blaisetiong-dev.workers.dev/preview/astro-marketing/og.png",
        page,
      ),
    ).toBe("/og.png");
    expect(
      previewSitePath(
        "https://astro-marketing.blaisetiong-dev.workers.dev/preview/astro-marketing/",
        page,
      ),
    ).toBe("/");
  });

  it("keeps site-root paths as paths", () => {
    expect(previewSitePath("/og.png", page)).toBe("/og.png");
    expect(previewSitePath("https://example.com/blog/welcome/", page)).toBe("/blog/welcome/");
  });
});

describe("localizePreviewAssetUrl", () => {
  const page = "http://127.0.0.1:51995/preview/astro-marketing/blog/";

  it("keeps assets already on the preview origin", () => {
    expect(
      localizePreviewAssetUrl(
        "http://127.0.0.1:51995/preview/astro-marketing/favicon.svg",
        page,
      ),
    ).toBe("http://127.0.0.1:51995/preview/astro-marketing/favicon.svg");
  });

  it("maps a published absolute URL onto the live preview origin by pathname", () => {
    expect(
      localizePreviewAssetUrl(
        "https://astro-marketing.blaisetiong-dev.workers.dev/preview/astro-marketing/og.png",
        page,
      ),
    ).toBe("http://127.0.0.1:51995/preview/astro-marketing/og.png");
  });

  it("resolves a site-root asset against the preview base path", () => {
    expect(localizePreviewAssetUrl("https://example.com/og.png", page)).toBe(
      "http://127.0.0.1:51995/preview/astro-marketing/og.png",
    );
  });

  it("maps a bare site-root path onto the framed preview mount", () => {
    expect(localizePreviewAssetUrl("/og.png", page)).toBe(
      "http://127.0.0.1:51995/preview/astro-marketing/og.png",
    );
    expect(localizePreviewAssetUrl("/favicon.svg", page)).toBe(
      "http://127.0.0.1:51995/preview/astro-marketing/favicon.svg",
    );
  });

  it("resolves relative hrefs against the page URL", () => {
    expect(localizePreviewAssetUrl("/preview/astro-marketing/favicon.svg", page)).toBe(
      "http://127.0.0.1:51995/preview/astro-marketing/favicon.svg",
    );
  });

  it("returns empty for missing href", () => {
    expect(localizePreviewAssetUrl("", page)).toBe("");
  });
});

describe("previewMediaSrc", () => {
  const page = "http://127.0.0.1:51995/preview/site-1/";

  it("rewrites preview-path assets onto the UI origin with the auth token", () => {
    expect(
      previewMediaSrc(
        "https://cdn.example/preview/site-1/og.png",
        page,
        { uiOrigin: "http://localhost:5173", token: "abc" },
      ),
    ).toBe("http://localhost:5173/preview/site-1/og.png?t=abc");
  });

  it("keeps non-preview remote URLs when they cannot be localized under /preview", () => {
    // Still localized onto the preview base; then proxied.
    expect(
      previewMediaSrc("https://cdn.example/brand.png", page, {
        uiOrigin: "http://localhost:5173",
        token: "abc",
      }),
    ).toBe("http://localhost:5173/preview/site-1/brand.png?t=abc");
  });

  it("falls back to the localized preview URL when no token is available", () => {
    expect(
      previewMediaSrc(
        "https://cdn.example/preview/site-1/og.png",
        page,
        { uiOrigin: "http://localhost:5173" },
      ),
    ).toBe("http://127.0.0.1:51995/preview/site-1/og.png");
  });
});
