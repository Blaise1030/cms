// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import {
  crashPrompt,
  pageErrorPrompt,
  readOverlay,
  watchPageErrors,
} from "./pageErrors";

function astroOverlay(): HTMLElement {
  const el = document.createElement("vite-error-overlay");
  const root = el.attachShadow({ mode: "open" });
  root.innerHTML = `
    <h1 id="title">Expected "}" but found "&lt;"</h1>
    <div id="message-content">Unexpected token in Hero.astro</div>
    <div id="hint-content">Check the closing brace.</div>
    <section id="code"><header><h2 title="/site/src/components/Hero.astro:12:4">components/Hero.astro:12:4</h2></header></section>
    <div id="stack-content">at parse (compiler.js:1:1)</div>`;
  return el;
}

describe("readOverlay", () => {
  it("reads Astro's overlay", () => {
    expect(readOverlay(astroOverlay())).toEqual({
      kind: "build",
      message: 'Expected "}" but found "<": Unexpected token in Hero.astro',
      file: "/site/src/components/Hero.astro:12:4",
      stack: "at parse (compiler.js:1:1)",
      hint: "Check the closing brace.",
    });
  });

  it("reads Vite's overlay", () => {
    const el = document.createElement("vite-error-overlay");
    el.attachShadow({ mode: "open" }).innerHTML = `
      <pre class="message-body">Failed to resolve import "./nope"</pre>
      <pre class="file">src/main.ts:3:8</pre>
      <pre class="stack"></pre>`;
    expect(readOverlay(el)).toMatchObject({
      message: 'Failed to resolve import "./nope"',
      file: "src/main.ts:3:8",
      stack: undefined,
    });
  });
});

describe("watchPageErrors", () => {
  it("reports an overlay appearing and going away", async () => {
    const onError = vi.fn();
    const onOverlayCleared = vi.fn();
    const detach = watchPageErrors(window, { onError, onOverlayCleared });
    const overlay = astroOverlay();
    document.body.append(overlay);
    await new Promise((r) => setTimeout(r));
    expect(onError).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "build" }),
    );
    overlay.remove();
    await new Promise((r) => setTimeout(r));
    expect(onOverlayCleared).toHaveBeenCalled();
    detach();
  });

  it("reports an overlay already on the page", () => {
    const overlay = astroOverlay();
    document.body.append(overlay);
    const onError = vi.fn();
    const detach = watchPageErrors(window, {
      onError,
      onOverlayCleared: vi.fn(),
    });
    expect(onError).toHaveBeenCalledTimes(1);
    detach();
    overlay.remove();
  });

  it("reports uncaught errors and rejections until detached", () => {
    const onError = vi.fn();
    const detach = watchPageErrors(window, {
      onError,
      onOverlayCleared: vi.fn(),
    });
    const boom = new TypeError("x is undefined");
    window.dispatchEvent(
      new ErrorEvent("error", {
        error: boom,
        message: boom.message,
        filename: "/src/app.ts",
        lineno: 4,
        colno: 2,
      }),
    );
    expect(onError).toHaveBeenLastCalledWith(
      expect.objectContaining({
        kind: "runtime",
        message: "TypeError: x is undefined",
        file: "/src/app.ts:4:2",
      }),
    );
    const rejection = new Event("unhandledrejection") as PromiseRejectionEvent;
    Object.defineProperty(rejection, "reason", { value: "nope" });
    window.dispatchEvent(rejection);
    expect(onError).toHaveBeenLastCalledWith({
      kind: "runtime",
      message: "nope",
    });
    detach();
    window.dispatchEvent(new ErrorEvent("error", { message: "late" }));
    expect(onError).toHaveBeenCalledTimes(2);
  });
});

describe("prompts", () => {
  it("describes a page error for the agent", () => {
    const prompt = pageErrorPrompt(readOverlay(astroOverlay()), "/about/");
    expect(prompt).toContain("The preview page /about/ fails to build:");
    expect(prompt).toContain("at /site/src/components/Hero.astro:12:4");
    expect(prompt).toContain("Hint: Check the closing brace.");
    expect(prompt).toMatch(/fix it\.$/);
  });

  it("describes a crash, with or without its output", () => {
    expect(crashPrompt("exit code 1\nCannot find module 'astro'")).toContain(
      "Cannot find module 'astro'",
    );
    expect(crashPrompt(undefined)).not.toContain("```");
  });
});
