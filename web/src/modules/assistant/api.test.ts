import { describe, expect, it, vi } from "vitest";
import { createApi, type FetchLike } from "@/core/api";
import { decodeEvent } from "@/core/socket";

const json = (body: unknown, init: ResponseInit = {}) =>
  new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
    ...init,
  });

const runtime = { phase: "ready", version: "1.0.0", source: "managed", progress: null, message: null };

function api(fetchImpl: FetchLike) {
  return createApi({ origin: "http://127.0.0.1:7800" }, fetchImpl);
}

describe("assistant setup routes", () => {
  it("reads the runtime snapshot", async () => {
    const fetchMock = vi.fn<FetchLike>(async () => json({ node: runtime, providers: [] }));
    await expect(api(fetchMock).setupRuntimes()).resolves.toEqual({ node: runtime, providers: [] });
    expect(fetchMock.mock.calls[0]![0]).toBe("http://127.0.0.1:7800/api/setup/runtimes");
    expect(fetchMock.mock.calls[0]![1]!.method).toBe("GET");
  });

  it("targets one runtime for download, cancel and retry", async () => {
    const fetchMock = vi.fn<FetchLike>(async () => json(runtime));
    const client = api(fetchMock);
    await client.downloadRuntime("my assistant");
    await client.cancelRuntime("my assistant");
    await client.retryRuntime("my assistant");
    expect(fetchMock.mock.calls.map(([url, init]) => `${init!.method} ${url}`)).toEqual([
      "POST http://127.0.0.1:7800/api/setup/runtimes/my%20assistant/download",
      "POST http://127.0.0.1:7800/api/setup/runtimes/my%20assistant/cancel",
      "POST http://127.0.0.1:7800/api/setup/runtimes/my%20assistant/retry",
    ]);
  });

  it("verifies a provider", async () => {
    const fetchMock = vi.fn<FetchLike>(async () => json({ chatReady: true, reason: null }));
    await expect(api(fetchMock).verifyProvider("opencode")).resolves.toEqual({
      chatReady: true,
      reason: null,
    });
    expect(fetchMock.mock.calls[0]![0]).toBe(
      "http://127.0.0.1:7800/api/setup/providers/opencode/verify",
    );
    expect(fetchMock.mock.calls[0]![1]!.method).toBe("POST");
  });

  it("starts a sign-in, submits a code and cancels", async () => {
    const fetchMock = vi.fn<FetchLike>(async () => json({ state: "browser", url: "https://x.test" }));
    const client = api(fetchMock);
    await client.signInProvider("claude");
    await client.submitProviderSignInCode("claude", "ABCD-1234");
    await client.cancelProviderSignIn("claude");
    expect(fetchMock.mock.calls.map(([url, init]) => `${init!.method} ${url}`)).toEqual([
      "POST http://127.0.0.1:7800/api/setup/providers/claude/sign-in",
      "POST http://127.0.0.1:7800/api/setup/providers/claude/sign-in/code",
      "POST http://127.0.0.1:7800/api/setup/providers/claude/cancel",
    ]);
    expect(JSON.parse(fetchMock.mock.calls[1]![1]!.body as string)).toEqual({ code: "ABCD-1234" });
  });

  it("signs a provider out", async () => {
    const fetchMock = vi.fn<FetchLike>(async () => json({ signedOut: true }));
    await expect(api(fetchMock).signOutProvider("claude")).resolves.toEqual({ signedOut: true });
    expect(fetchMock.mock.calls[0]![0]).toBe(
      "http://127.0.0.1:7800/api/setup/providers/claude/sign-out",
    );
  });
});

describe("setup.runtime wire event", () => {
  it("decodes a progress event", () => {
    expect(
      decodeEvent(
        JSON.stringify({
          type: "setup.runtime",
          runtime: "opencode",
          phase: "downloading",
          progress: 42,
        }),
      ),
    ).toMatchObject({ type: "setup.runtime", runtime: "opencode", phase: "downloading", progress: 42 });
  });

  it("drops an event without a runtime or phase", () => {
    expect(decodeEvent({ type: "setup.runtime", phase: "ready" })).toBeNull();
    expect(decodeEvent({ type: "setup.runtime", runtime: "opencode" })).toBeNull();
  });
});
