// @vitest-environment happy-dom
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
// The page side: the bridge the orso Astro integration injects into every preview page.
import { readFileSync } from "node:fs";
const bridgeSource = readFileSync(`${process.cwd()}/../packages/astro/src/bridge.js`, "utf8");
import { isBridgeMessage } from "./frameBridge";
import { crashPrompt, type PageError, pageErrorPrompt } from "./pageErrors";

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

const APP = "http://orso.localhost:7800";
const settle = () => new Promise((r) => setTimeout(r));

describe("the bridge reports page errors to the app", () => {
  const posted: { message: unknown; origin: string }[] = [];
  const last = () => posted.at(-1)!.message as Record<string, unknown>;

  beforeAll(() => {
    // A top-level window is its own parent: what the bridge posts to `parent` lands here.
    vi.spyOn(window, "postMessage").mockImplementation(((message: unknown, origin: string) => {
      posted.push({ message, origin });
    }) as typeof window.postMessage);
    new Function(`const __orsoAppOrigins = ${JSON.stringify([APP])};\n${bridgeSource}`)();
  });
  afterAll(() => vi.restoreAllMocks());

  it("reads Astro's overlay when it appears and says when it goes away", async () => {
    const overlay = astroOverlay();
    document.body.append(overlay);
    await settle();
    expect(isBridgeMessage(last())).toBe(true);
    expect(last()).toEqual({
      type: "orso:preview",
      op: "error",
      error: {
        kind: "build",
        message: 'Expected "}" but found "<": Unexpected token in Hero.astro',
        file: "/site/src/components/Hero.astro:12:4",
        stack: "at parse (compiler.js:1:1)",
        hint: "Check the closing brace.",
      },
    });
    expect(posted.at(-1)!.origin).toBe(APP);
    overlay.remove();
    await settle();
    expect(last()).toEqual({ type: "orso:preview", op: "error-cleared" });
  });

  it("reads Vite's overlay", async () => {
    const el = document.createElement("vite-error-overlay");
    el.attachShadow({ mode: "open" }).innerHTML = `
      <pre class="message-body">Failed to resolve import "./nope"</pre>
      <pre class="file">src/main.ts:3:8</pre>
      <pre class="stack"></pre>`;
    document.body.append(el);
    await settle();
    expect((last().error as PageError)).toEqual({
      kind: "build",
      message: 'Failed to resolve import "./nope"',
      file: "src/main.ts:3:8",
    });
    el.remove();
    await settle();
  });

  it("reports uncaught errors and rejections", () => {
    const boom = new TypeError("x is undefined");
    window.dispatchEvent(
      new ErrorEvent("error", { error: boom, message: boom.message, filename: "/src/app.ts", lineno: 4, colno: 2 }),
    );
    expect(isBridgeMessage(last())).toBe(true);
    expect(last().error).toMatchObject({
      kind: "runtime",
      message: "TypeError: x is undefined",
      file: "/src/app.ts:4:2",
    });
    const rejection = new Event("unhandledrejection") as PromiseRejectionEvent;
    Object.defineProperty(rejection, "reason", { value: "nope" });
    window.dispatchEvent(rejection);
    expect(last().error).toEqual({ kind: "runtime", message: "nope" });
    // Every report went to the app's origin by name.
    expect(posted.every((p) => p.origin === APP)).toBe(true);
  });
});

describe("prompts", () => {
  it("describes a page error for the agent", () => {
    const prompt = pageErrorPrompt(
      {
        kind: "build",
        message: 'Expected "}" but found "<": Unexpected token in Hero.astro',
        file: "/site/src/components/Hero.astro:12:4",
        stack: "at parse (compiler.js:1:1)",
        hint: "Check the closing brace.",
      },
      "/about/",
    );
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
