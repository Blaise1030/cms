import { describe, expect, it } from "vitest";
import {
  STORAGE_KEY,
  parseDaemonLink,
  resolveDaemonConfig,
  saveDaemonConfig,
  type ResolveDeps,
} from "./config";

function memoryStorage(initial?: Record<string, string>) {
  const data = new Map(Object.entries(initial ?? {}));
  return {
    getItem: (k: string) => data.get(k) ?? null,
    setItem: (k: string, v: string) => void data.set(k, v),
    data,
  };
}

function deps(search: string, storage = memoryStorage()) {
  const replaced: string[] = [];
  const d: ResolveDeps = {
    location: { origin: "http://127.0.0.1:7800", search, pathname: "/", hash: "" },
    storage,
    replaceUrl: (u) => replaced.push(u),
  };
  return { d, storage, replaced };
}

describe("resolveDaemonConfig", () => {
  it("defaults to the page origin when embedded", () => {
    const { d, replaced } = deps("");
    expect(resolveDaemonConfig(d)).toEqual({ origin: "http://127.0.0.1:7800" });
    expect(replaced).toEqual([]);
  });

  it("consumes ?t= and strips it from the URL", () => {
    const { d, storage, replaced } = deps("?t=abc&x=1");
    expect(resolveDaemonConfig(d)).toEqual({ origin: "http://127.0.0.1:7800", token: "abc" });
    expect(replaced).toEqual(["/?x=1"]);
    expect(JSON.parse(storage.data.get(STORAGE_KEY)!)).toEqual({ token: "abc" });
  });

  it("uses ?daemon= for a hosted app and persists it", () => {
    const first = deps("?daemon=http://localhost:9000/foo&t=xyz");
    expect(resolveDaemonConfig(first.d)).toEqual({ origin: "http://localhost:9000", token: "xyz" });
    const second = deps("", first.storage);
    expect(resolveDaemonConfig(second.d)).toEqual({
      origin: "http://localhost:9000",
      token: "xyz",
    });
  });

  it("drops the old token when switching daemon without a new one", () => {
    const storage = memoryStorage();
    saveDaemonConfig(storage, { origin: "http://old:1", token: "old" }, "http://127.0.0.1:7800");
    const next = deps("?daemon=http://new:2", storage);
    expect(resolveDaemonConfig(next.d)).toEqual({ origin: "http://new:2" });
  });

  it("ignores corrupt storage and invalid ?daemon=", () => {
    const { d } = deps("?daemon=javascript:alert(1)", memoryStorage({ [STORAGE_KEY]: "{oops" }));
    expect(resolveDaemonConfig(d)).toEqual({ origin: "http://127.0.0.1:7800" });
  });
});

describe("parseDaemonLink", () => {
  it("extracts origin and token", () => {
    expect(parseDaemonLink(" http://127.0.0.1:7800/?t=abc ")).toEqual({
      origin: "http://127.0.0.1:7800",
      token: "abc",
    });
    expect(parseDaemonLink("https://d.example")).toEqual({ origin: "https://d.example" });
  });

  it("rejects non-http links", () => {
    expect(parseDaemonLink("nope")).toBeNull();
    expect(parseDaemonLink("ftp://x")).toBeNull();
  });
});
