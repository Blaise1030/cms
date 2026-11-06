import { describe, expect, it } from "vitest";
import type { UiChunk } from "@/core/types";
import { buildChatStreamUrl, createChatStream, type ChatStreamSource } from "./stream";

const config = { origin: "http://127.0.0.1:7800" };

function fakeSource() {
  const created: { url: string; source: ChatStreamSource; open(): void; frame(data: string): void; fail(): void }[] = [];
  const create = (url: string) => {
    const source: ChatStreamSource = { onopen: null, onmessage: null, onerror: null, close: () => void (closed += 1) };
    let closed = 0;
    created.push({
      url,
      source,
      open: () => source.onopen?.({}),
      frame: (data: string) => source.onmessage?.({ data }),
      fail: () => source.onerror?.({}),
    });
    return source;
  };
  return { create, created };
}

const chunk = (text: string): UiChunk => ({ type: "text_delta", session_id: "s", text });

describe("buildChatStreamUrl", () => {
  it("targets the chat stream on the daemon, with the token when there is one", () => {
    expect(buildChatStreamUrl({ ...config, token: "a+b" }, "proj", "chat-1")).toBe(
      "http://127.0.0.1:7800/api/chat/proj/chat-1/stream?t=a%2Bb",
    );
    expect(buildChatStreamUrl(config, "proj", "chat-1")).toBe(
      "http://127.0.0.1:7800/api/chat/proj/chat-1/stream",
    );
  });

  it("keeps path segments from escaping the route", () => {
    expect(buildChatStreamUrl(config, "a/b", "c?d")).toContain("/api/chat/a%2Fb/c%3Fd/stream");
  });
});

describe("createChatStream", () => {
  it("skips the history the panel already rendered on its first connection, then appends", () => {
    const fake = fakeSource();
    const seen: UiChunk[] = [];
    const stream = createChatStream({
      url: () => "ws://unused",
      skipOnFirstOpen: () => 2,
      onReset: () => seen.push({ type: "user_text", session_id: "s", text: "reset" }),
      onChunk: (c) => seen.push(c),
      createSource: fake.create,
    });
    stream.connect();
    fake.created[0]!.open();
    fake.created[0]!.frame(JSON.stringify(chunk("old")));
    fake.created[0]!.frame(JSON.stringify(chunk("old")));
    fake.created[0]!.frame(JSON.stringify(chunk("new")));
    expect(seen).toEqual([chunk("new")]);
  });

  it("rebuilds from the full replay after a reconnect", () => {
    const fake = fakeSource();
    const seen: string[] = [];
    const stream = createChatStream({
      url: () => "u",
      skipOnFirstOpen: () => 1,
      onReset: () => seen.push("reset"),
      onChunk: (c) => seen.push("text" in c ? c.text : c.type),
      createSource: fake.create,
    });
    stream.connect();
    fake.created[0]!.open();
    fake.created[0]!.frame(JSON.stringify(chunk("already-there")));
    fake.created[0]!.open(); // EventSource reconnected: the daemon replays everything.
    fake.created[0]!.frame(JSON.stringify(chunk("again")));
    expect(seen).toEqual(["reset", "again"]);
  });

  it("ignores frames that are not a known chunk", () => {
    const fake = fakeSource();
    const seen: UiChunk[] = [];
    const stream = createChatStream({
      url: () => "u",
      onChunk: (c) => seen.push(c),
      createSource: fake.create,
    });
    stream.connect();
    fake.created[0]!.open();
    fake.created[0]!.frame("not json");
    fake.created[0]!.frame(JSON.stringify({ hello: "world" }));
    fake.created[0]!.frame(JSON.stringify(chunk("ok")));
    expect(seen).toEqual([chunk("ok")]);
  });

  it("reports connection errors without tearing the source down (EventSource retries)", () => {
    const fake = fakeSource();
    let errors = 0;
    const stream = createChatStream({ url: () => "u", onError: () => (errors += 1), createSource: fake.create });
    stream.connect();
    fake.created[0]!.fail();
    fake.created[0]!.open();
    expect(errors).toBe(1);
    expect(fake.created).toHaveLength(1);
  });

  it("closes the source and ignores later frames", () => {
    const fake = fakeSource();
    const seen: UiChunk[] = [];
    const stream = createChatStream({ url: () => "u", onChunk: (c) => seen.push(c), createSource: fake.create });
    stream.connect();
    fake.created[0]!.open();
    stream.close();
    fake.created[0]!.frame(JSON.stringify(chunk("late")));
    expect(seen).toEqual([]);
  });

  it("connects once, even when connect is called twice", () => {
    const fake = fakeSource();
    const stream = createChatStream({ url: () => "u", createSource: fake.create });
    expect(stream.connect()).toBe(true);
    expect(stream.connect()).toBe(true);
    expect(fake.created).toHaveLength(1);
  });

  it("reports that it could not attach when the source cannot be created", () => {
    const stream = createChatStream({
      url: () => "u",
      createSource: () => {
        throw new Error("no EventSource");
      },
    });
    expect(stream.connect()).toBe(false);
  });
});
