import { describe, expect, it, vi } from "vitest";
import { ApiError, NetworkError, createApi, mapErrorResponse, type FetchLike } from "./api";

const json = (body: unknown, init: ResponseInit = {}) =>
  new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
    ...init,
  });

function api(fetchImpl: FetchLike, token?: string) {
  return createApi({ origin: "http://127.0.0.1:7800", token }, fetchImpl);
}

describe("mapErrorResponse", () => {
  it("maps the daemon envelope", () => {
    const err = mapErrorResponse(
      409,
      "Conflict",
      JSON.stringify({ error: { code: "CONFLICT", message: "name taken" } }),
    );
    expect(err).toBeInstanceOf(ApiError);
    expect(err.code).toBe("CONFLICT");
    expect(err.message).toBe("name taken");
    expect(err.status).toBe(409);
  });

  it("flags unauthorized errors", () => {
    const err = mapErrorResponse(401, "", '{"error":{"code":"UNAUTH","message":"bad token"}}');
    expect(err.isUnauthorized).toBe(true);
  });

  it("falls back to UNKNOWN for unknown codes and non-JSON bodies", () => {
    expect(mapErrorResponse(500, "Oops", '{"error":{"code":"NOPE","message":"x"}}').code).toBe(
      "UNKNOWN",
    );
    const html = mapErrorResponse(502, "Bad Gateway", "<html>bad gateway</html>");
    expect(html.code).toBe("UNKNOWN");
    expect(html.message).toBe("Bad Gateway");
    expect(mapErrorResponse(503, "", "").message).toBe("HTTP 503");
  });
});

describe("createApi", () => {
  it("sends bearer auth, JSON bodies and unwraps list envelopes", async () => {
    const fetchMock = vi.fn<FetchLike>(async () => json({ projects: [{ id: "a" }] }));
    const projects = await api(fetchMock, "tok").listProjects();
    expect(projects).toEqual([{ id: "a" }]);
    const [url, init] = fetchMock.mock.calls[0]!;
    expect(url).toBe("http://127.0.0.1:7800/api/projects");
    expect((init!.headers as Record<string, string>).Authorization).toBe("Bearer tok");

    fetchMock.mockResolvedValueOnce(json({ id: "b" }));
    await api(fetchMock).createProject("Site");
    const [, post] = fetchMock.mock.calls[1]!;
    expect(post!.method).toBe("POST");
    expect(post!.body).toBe('{"name":"Site"}');
    expect((post!.headers as Record<string, string>)["Content-Type"]).toBe("application/json");
    expect((post!.headers as Record<string, string>).Authorization).toBeUndefined();
  });

  it("requires confirm on delete", async () => {
    const fetchMock = vi.fn<FetchLike>(async () => json({ deleted: "p" }));
    await api(fetchMock).deleteProject("p1");
    expect(fetchMock.mock.calls[0]![0]).toBe(
      "http://127.0.0.1:7800/api/projects/p1?confirm=true",
    );
    expect(fetchMock.mock.calls[0]![1]!.method).toBe("DELETE");
  });

  it("reads diffs as text", async () => {
    const fetchMock = vi.fn<FetchLike>(async () => new Response("diff --git a b"));
    expect(await api(fetchMock).checkpointDiff("p", "abc")).toBe("diff --git a b");
  });

  it("throws ApiError from the error envelope", async () => {
    const fetchMock = vi.fn<FetchLike>(async () =>
      json({ error: { code: "ENOENT", message: "no such project" } }, { status: 404 }),
    );
    await expect(api(fetchMock).tree("nope")).rejects.toMatchObject({
      name: "ApiError",
      code: "ENOENT",
      status: 404,
      message: "no such project",
    });
  });

  it("throws NetworkError when the daemon cannot be reached", async () => {
    const fetchMock = vi.fn<FetchLike>(async () => {
      throw new TypeError("Failed to fetch");
    });
    const err = await api(fetchMock)
      .health()
      .catch((e: unknown) => e);
    expect(err).toBeInstanceOf(NetworkError);
    expect((err as NetworkError).message).toContain("127.0.0.1:7800");
  });

  it("picks up config changes when given a getter", async () => {
    const cfg = { origin: "http://a.test", token: undefined as string | undefined };
    const fetchMock = vi.fn<FetchLike>(async () => json({ status: "ok", version: "1" }));
    const a = createApi(() => cfg, fetchMock);
    await a.health();
    cfg.origin = "http://b.test";
    cfg.token = "t";
    await a.health();
    expect(fetchMock.mock.calls[0]![0]).toBe("http://a.test/api/health");
    expect(fetchMock.mock.calls[1]![0]).toBe("http://b.test/api/health");
    expect(a.previewUrl("my proj")).toBe("http://b.test/preview/my%20proj/");
  });

  it("builds chat routes and auth headers", async () => {
    const fetchMock = vi.fn<FetchLike>(async () => json({ chats: [], running: null }));
    const a = api(fetchMock, "tok");
    await a.listChats("my proj");
    fetchMock.mockResolvedValueOnce(json({ chunks: [], provider: "claude", model: null, running: false }));
    await a.chatHistory("my proj", "c1");
    fetchMock.mockResolvedValueOnce(json({ id: "c2", provider: "claude", live: false, running: false, updatedAt: 0 }));
    await a.createChat("my proj", "claude", "sonnet");
    fetchMock.mockResolvedValueOnce(json({ accepted: true }));
    await a.sendChatMessage("my proj", "c2", "hello");
    fetchMock.mockResolvedValueOnce(json({}));
    await a.stopChat("my proj", "c2");
    expect(fetchMock.mock.calls.map(([url, init]) => `${init!.method} ${url}`)).toEqual([
      "GET http://127.0.0.1:7800/api/chat/my%20proj",
      "GET http://127.0.0.1:7800/api/chat/my%20proj/c1",
      "POST http://127.0.0.1:7800/api/chat/my%20proj",
      "POST http://127.0.0.1:7800/api/chat/my%20proj/c2/message",
      "POST http://127.0.0.1:7800/api/chat/my%20proj/c2/stop",
    ]);
    expect(JSON.parse(fetchMock.mock.calls[2]![1]!.body as string)).toEqual({ provider: "claude", model: "sonnet" });
    expect(JSON.parse(fetchMock.mock.calls[3]![1]!.body as string)).toEqual({ text: "hello" });
    expect(a.attachmentUrl("my proj", "a.png")).toBe(
      "http://127.0.0.1:7800/api/chat/my%20proj/attachments/a.png",
    );
  });

  it("accepts the empty 202 body from sending a chat message", async () => {
    const fetchMock = vi.fn<FetchLike>(async () => new Response(null, { status: 202 }));
    await expect(api(fetchMock).sendChatMessage("p", "c1", "hello")).resolves.toBeUndefined();
  });

  it("accepts the empty 204 body from stopping a chat", async () => {
    const fetchMock = vi.fn<FetchLike>(async () => new Response(null, { status: 204 }));
    await expect(api(fetchMock).stopChat("p", "c1")).resolves.toBeUndefined();
  });
});

describe("updateContentEntry", () => {
  const body = { project: "p", collection: "blog", id: "welcome", branch: "content/a", hash: "h1", fields: {}, body: "![A](orso-image:c1)\n" };

  it("sends JSON when the save has no images", async () => {
    const fetchImpl = vi.fn<FetchLike>(async () => json({ id: "welcome" }));
    await api(fetchImpl).updateContentEntry(body);
    const init = fetchImpl.mock.calls[0]![1]!;
    expect((init.headers as Record<string, string>)["Content-Type"]).toBe("application/json");
    expect(JSON.parse(init.body as string)).toEqual(body);
  });

  it("sends multipart with a JSON entry part and one part per image keyed by client ID", async () => {
    const fetchImpl = vi.fn<FetchLike>(async () => json({ id: "welcome" }));
    const file = new Blob([new Uint8Array([1, 2, 3])], { type: "image/png" });
    await api(fetchImpl, "tok").updateContentEntry(body, [{ clientId: "c1", alt: "A", filename: "a.png", file }]);
    const init = fetchImpl.mock.calls[0]![1]!;
    const headers = init.headers as Record<string, string>;
    expect(headers["Content-Type"]).toBeUndefined();
    expect(headers.Authorization).toBe("Bearer tok");
    const form = init.body as FormData;
    expect(JSON.parse(await (form.get("entry") as Blob).text())).toEqual({ ...body, images: [{ clientId: "c1", alt: "A", filename: "a.png" }] });
    const part = form.get("c1") as File;
    expect(part.name).toBe("a.png");
    expect(new Uint8Array(await part.arrayBuffer())).toEqual(new Uint8Array([1, 2, 3]));
  });
});
