import { describe, expect, it } from "vitest";
import { DAEMON_NAVIGATION_DENYLIST, pwaOptions } from "../../pwa.config";

// Workbox tests the denylist against `pathname + search`.
const blocked = (url: string) => DAEMON_NAVIGATION_DENYLIST.some((re) => re.test(url));

describe("pwa config", () => {
  it("never registers runtime caching for live daemon data", () => {
    expect(pwaOptions.workbox?.runtimeCaching ?? []).toEqual([]);
  });

  it("keeps daemon-served navigations out of the SPA fallback", () => {
    for (const url of ["/api/projects", "/ws/events", "/preview/abc/", "/preview/abc/about", "/?t=deadbeef", "/?x=1&t=ab"]) {
      expect(blocked(url), url).toBe(true);
    }
  });

  it("serves the app shell for ordinary routes", () => {
    for (const url of ["/", "/p/abc", "/apiary", "/preview-notes"]) {
      expect(blocked(url), url).toBe(false);
    }
  });
});
