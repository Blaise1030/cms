// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { attachPicker, describeElement, elementLabel, elementPrompt } from "./elementPicker";

afterEach(() => {
  document.body.innerHTML = "";
});

describe("describeElement", () => {
  it("points at the Astro source file, relative to the project", () => {
    document.body.innerHTML = `
      <main>
        <section class="hero astro-x7k2pd" data-astro-source-file="/home/me/orso-projects/site-1/src/components/Hero.astro" data-astro-source-loc="12:3">
          <h1 class="title astro-x7k2pd">Build   faster
            sites</h1>
        </section>
      </main>`;
    const h1 = document.querySelector("h1")!;
    const pick = describeElement(h1, "site-1");
    expect(pick).toEqual({
      label: "h1.title",
      tag: '<h1 class="title">',
      selector: "body > main > section > h1",
      text: "Build faster sites",
      file: "src/components/Hero.astro",
      loc: "12:3",
    });
    expect(elementPrompt(pick)).toBe(
      '[element <h1 class="title"> "Build faster sites" in src/components/Hero.astro:12:3]',
    );
  });

  it("falls back to a CSS path from the nearest id without source annotations", () => {
    document.body.innerHTML = `<div id="pricing"><ul><li>A</li><li><a href="/buy">Buy</a></li></ul></div>`;
    const pick = describeElement(document.querySelector("a")!, "site-1");
    expect(pick.selector).toBe("#pricing > ul > li:nth-of-type(2) > a");
    expect(pick.file).toBeUndefined();
    expect(elementPrompt(pick)).toBe(
      '[element <a href="/buy"> "Buy" at `#pricing > ul > li:nth-of-type(2) > a`]',
    );
  });

  it("puts the screenshot path after the reference, escaped, so the agent attaches it", () => {
    const pick = { tag: "<h1>", selector: "h1", text: "Hi", label: "h1", screenshot: "/state/My Site/a.png" };
    expect(elementPrompt(pick)).toBe('[element <h1> "Hi" at `h1`] /state/My\\ Site/a.png ');
  });

  it("labels by id, then by up to two classes", () => {
    document.body.innerHTML = `<nav id="top" class="a"></nav><p class="a b c"></p>`;
    expect(elementLabel(document.querySelector("nav")!)).toBe("nav#top");
    expect(elementLabel(document.querySelector("p")!)).toBe("p.a.b");
  });
});

describe("attachPicker", () => {
  it("highlights on hover, picks on click without letting the page see it, and cleans up", () => {
    document.body.innerHTML = `<a href="#x">Link</a>`;
    const link = document.querySelector("a")!;
    const pageClick = vi.fn();
    link.addEventListener("click", pageClick);
    const onPick = vi.fn();
    const detach = attachPicker(window, onPick, vi.fn());

    link.dispatchEvent(new MouseEvent("mousemove", { bubbles: true }));
    const overlay = document.getElementById("__orso-picker")!;
    expect(overlay.style.display).toBe("block");
    expect(overlay.textContent).toBe("a");

    link.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    expect(onPick).toHaveBeenCalledWith(link);
    expect(pageClick).not.toHaveBeenCalled();

    detach();
    expect(document.getElementById("__orso-picker")).toBeNull();
    link.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    expect(pageClick).toHaveBeenCalledOnce();
    expect(onPick).toHaveBeenCalledOnce();
  });

  it("cancels on Escape", () => {
    const onCancel = vi.fn();
    const detach = attachPicker(window, vi.fn(), onCancel);
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(onCancel).toHaveBeenCalledOnce();
    detach();
  });
});
