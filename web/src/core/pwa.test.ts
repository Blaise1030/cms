import { describe, expect, it } from "vitest";
import {
  DAEMON_NAVIGATION_DENYLIST,
  isShellEntry,
  pwaOptions,
  shellScripts,
} from "../../pwa.config";

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

  describe("shell-only precache", () => {
    const html = `<head>
      <script type="module" crossorigin src="/assets/index-A.js"></script>
      <link rel="modulepreload" crossorigin href="/assets/vendor-B.js">
      <link rel="stylesheet" crossorigin href="/assets/index-C.css">
      <script id="vite-plugin-pwa:register-sw" src="/registerSW.js"></script></head>`;

    it("finds the entry script and its modulepreload deps", () => {
      expect([...shellScripts(html)].sort()).toEqual(["assets/index-A.js", "assets/vendor-B.js"]);
    });

    it("drops lazy chunks (shiki grammars/themes/wasm) but keeps shell files", () => {
      const shell = shellScripts(html);
      for (const lazy of ["assets/cpp-X.js", "assets/emacs-lisp-X.js", "assets/wasm-X.js", "assets/dist-X.js"]) {
        expect(isShellEntry(lazy, shell), lazy).toBe(false);
      }
      for (const kept of ["assets/index-A.js", "assets/vendor-B.js", "assets/index-C.css", "index.html", "icons/icon-192.png", "manifest.webmanifest", "registerSW.js"]) {
        expect(isShellEntry(kept, shell), kept).toBe(true);
      }
    });
  });
});
