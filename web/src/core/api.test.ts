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
});
