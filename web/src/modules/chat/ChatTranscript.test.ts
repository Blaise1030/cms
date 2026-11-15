// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import { h, nextTick } from "vue";
import type { UIMessage } from "ai";
import ChatTranscript from "./ChatTranscript.vue";

enableAutoUnmount(afterEach);

/** Each rendered message is 100px tall; the viewport shows 300px. happy-dom has no layout. */
const ROW = 100;
const VIEW = 300;

const msg = (id: string, role: UIMessage["role"] = "assistant", text = id): UIMessage =>
  ({ id, role, parts: [{ type: "text", text }] }) as UIMessage;
const range = (from: number, to: number) => Array.from({ length: to - from }, (_, i) => msg(`m${from + i}`));

const realStyle = window.getComputedStyle;
beforeEach(() => {
  // UChatMessages looks for its scroll parent by computed overflow; happy-dom has no stylesheet.
  vi.spyOn(window, "getComputedStyle").mockImplementation((el: Element, pseudo?: string | null) => {
    const style = realStyle.call(window, el, pseudo);
    if ((el as HTMLElement).dataset?.testid === "chat-viewport") {
      return new Proxy(style, { get: (t, k) => (k === "overflowY" ? "auto" : Reflect.get(t, k)) });
    }
    return style;
  });
});
afterEach(() => vi.restoreAllMocks());

async function mountTranscript(messages: UIMessage[], streaming = false) {
  const w = mount(ChatTranscript, {
    props: { messages, streaming, canLoadEarlier: true },
    slots: {
      default: ({ message }: { message: UIMessage }) =>
        h("div", { "data-testid": "row" }, (message.parts[0] as { text: string }).text),
    },
    attachTo: document.body,
  });
  const el = w.get("[data-testid='chat-viewport']").element as HTMLElement;
  Object.defineProperty(el, "scrollHeight", {
    configurable: true,
    get: () => el.querySelectorAll("[data-testid='row']").length * ROW,
  });
  Object.defineProperty(el, "clientHeight", { configurable: true, get: () => VIEW });
  el.scrollTo = ((opts: ScrollToOptions) => {
    el.scrollTop = opts.top ?? el.scrollTop;
  }) as typeof el.scrollTo;
  await flushPromises();
  return { w, el };
}

async function scrollTo(el: HTMLElement, top: number) {
  el.scrollTop = top;
  el.dispatchEvent(new Event("scroll"));
  await flushPromises();
}

describe("ChatTranscript", () => {
  it("renders every message through the slot inside Nuxt UI's message list", async () => {
    const { w } = await mountTranscript(range(0, 3));
    expect(w.findAll("[data-testid='row']").map((r) => r.text())).toEqual(["m0", "m1", "m2"]);
    expect(w.find("[data-testid='chat-viewport'] [data-testid='chat-messages']").exists()).toBe(true);
  });

  it("asks for earlier history near the top, and only when there is some", async () => {
    const { w, el } = await mountTranscript(range(0, 10));
    await scrollTo(el, 500);
    expect(w.emitted("loadEarlier")).toBeUndefined();
    await scrollTo(el, 40);
    expect(w.emitted("loadEarlier")).toHaveLength(1);
    await w.setProps({ canLoadEarlier: false });
    await scrollTo(el, 0);
    expect(w.emitted("loadEarlier")).toHaveLength(1);
  });

  it("keeps the visible messages in place when earlier history is prepended", async () => {
    const { w, el } = await mountTranscript(range(10, 20));
    await scrollTo(el, 30);
    // 1000px of content, 970px below the top of the viewport.
    await w.setProps({ messages: range(0, 20) });
    await nextTick();
    expect(el.scrollTop).toBe(2000 - 970);
  });

  it("keeps up with new content while the viewport is at the bottom", async () => {
    const { w, el } = await mountTranscript(range(0, 5));
    await scrollTo(el, 500 - VIEW);
    await w.setProps({ messages: range(0, 6) });
    await nextTick();
    expect(el.scrollTop).toBe(600);
  });

  it("stays put when the user has scrolled up, until they send a message", async () => {
    const { w, el } = await mountTranscript(range(0, 10));
    await scrollTo(el, 100);
    await w.setProps({ messages: range(0, 11) });
    await nextTick();
    expect(el.scrollTop).toBe(100);
    await w.setProps({ messages: [...range(0, 11), msg("u1", "user")] });
    await nextTick();
    expect(el.scrollTop).toBe(1200);
  });

  it("follows a streaming reply, and stops following once the user scrolls up", async () => {
    const { w, el } = await mountTranscript(range(0, 5), true);
    await scrollTo(el, 500 - VIEW);
    // The streamed reply grows in place: the same message list, new content.
    const grow = async (n: number) => {
      await w.setProps({ messages: [...range(0, 5), ...range(5, 5 + n)] });
      await flushPromises();
      // Browsers fire `scroll` for programmatic scrolling too; happy-dom does not.
      el.dispatchEvent(new Event("scroll"));
      await flushPromises();
    };
    await grow(2);
    expect(el.scrollTop).toBe(700);
    await scrollTo(el, 200);
    await grow(4);
    expect(el.scrollTop).toBe(200);
  });

  it("offers a jump to the latest message when scrolled away from the end", async () => {
    const { w, el } = await mountTranscript(range(0, 10), true);
    await scrollTo(el, 1000 - VIEW);
    expect(w.find("[aria-label='Scroll to bottom']").exists()).toBe(false);
    await scrollTo(el, 100);
    await w.setProps({ messages: range(0, 11) });
    await flushPromises();
    const jump = w.find("[aria-label='Scroll to bottom']");
    expect(jump.exists()).toBe(true);
    await jump.trigger("click");
    expect(el.scrollTop).toBe(1100);
  });
});
