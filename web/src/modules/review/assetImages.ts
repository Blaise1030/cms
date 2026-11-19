import type { InjectionKey } from "vue";

/** Rewrites a markdown text's review images (see `rewriteAssetImages`); identity when absent. */
export const ASSET_IMAGES: InjectionKey<(markdown: string) => string> = Symbol("review-asset-images");

const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/**
 * Points the images orso put on `repo`'s `orso-review-assets` branch
 * (`https://github.com/{repo}/blob/{sha}/{shots|uploads/…}?raw=true`) at `toUrl(sha, path)`, the
 * daemon's proxy, so they load in orso for private repositories too. Other images are left alone.
 */
export function rewriteAssetImages(markdown: string, repo: string, toUrl: (sha: string, path: string) => string): string {
  const pattern = new RegExp(
    `(!\\[[^\\]]*\\]\\()https://github\\.com/${escape(repo)}/blob/([0-9a-f]{7,40})/((?:shots|uploads)/[^)\\s?#]+)\\?raw=true(\\))`,
    "gi",
  );
  return markdown.replace(pattern, (_m, open: string, sha: string, path: string, close: string) => `${open}${toUrl(sha, path)}${close}`);
}

/** `owner/name` from a pull request URL (`https://github.com/owner/name/pull/7`). */
export function repoOfPullUrl(url: string | null | undefined): string | null {
  const m = url?.match(/^https:\/\/github\.com\/([^/]+\/[^/]+)\/pull\/\d+/i);
  return m ? m[1]! : null;
}
