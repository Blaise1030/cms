import assert from "node:assert/strict";
import http from "node:http";
import { after, test } from "node:test";
import { format } from "node:util";

import { z } from "astro/zod";
import { createEndpoint } from "orso-astro";

import { createCollectionSchemas } from "./fixtures/schemas.mjs";

const schemas = createCollectionSchemas({ z });
const TOKEN = "test-token-4f0c1d";
const MAX_BODY = 2 * 1024 * 1024;

/** Collection metadata the real integration will pass in from the manifest. */
const collections = {
  blog: { supportedForEditing: true, kind: "markdown", schema: schemas.blog, reason: null },
  helpDocs: { supportedForEditing: true, kind: "markdown", schema: schemas.helpDocs, reason: null },
  readOnly: {
    supportedForEditing: false,
    kind: "markdown",
    schema: schemas.blog,
    reason: "Collection uses plain glob() and cannot be edited",
  },
  broken: {
    supportedForEditing: true,
    kind: "markdown",
    schema: { safeParse: () => { throw new Error("schema exploded"); } },
    reason: null,
  },
  leaky: {
    supportedForEditing: true,
    kind: "markdown",
    schema: {
      safeParse: () => {
        const error = new Error("schema rejected input");
        // Custom properties can hold body-derived values; they must never be logged.
        error.details = { fields: { marker: "body-detail-marker" } };
        throw error;
      },
    },
    reason: null,
  },
};

const lookup = (name) => collections[name];

const endpoint = createEndpoint({ token: TOKEN, lookup });
let lastRequestHeaders;
const server = http.createServer((req, res) => {
  lastRequestHeaders = req.headers;
  endpoint(req, res, () => {
    res.statusCode = 418;
    res.end("next");
  });
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const origin = `http://127.0.0.1:${server.address().port}`;
after(() => {
  server.closeAllConnections?.();
  return new Promise((resolve) => server.close(resolve));
});

const post = (body, { token = TOKEN, method = "POST", contentType = "application/json", path = "/__orso", headers = {} } = {}) =>
  fetch(origin + path, {
    method,
    headers: {
      "content-type": contentType,
      "x-orso-content-token": token,
      ...headers,
    },
    body: method === "GET" ? undefined : typeof body === "string" ? body : JSON.stringify(body),
  });

const json = async (response) => ({ status: response.status, body: await response.json(), headers: response.headers });

/**
 * Send a chunked request with no Content-Length so the server must enforce the
 * streamed body cap, and keep writing after the 413 so a reset would surface.
 */
function streamPost(payload, { chunkSize = 64 * 1024 } = {}) {
  return new Promise((resolve, reject) => {
    const request = http.request(
      {
        host: "127.0.0.1",
        port: server.address().port,
        path: "/__orso",
        method: "POST",
        headers: { "content-type": "application/json", "x-orso-content-token": TOKEN },
      },
      (response) => {
        const chunks = [];
        response.on("data", (chunk) => chunks.push(chunk));
        response.on("end", () =>
          resolve({
            status: response.statusCode,
            headers: response.headers,
            body: JSON.parse(Buffer.concat(chunks).toString("utf8")),
          }),
        );
        response.on("error", reject);
      },
    );
    let offset = 0;
    const pump = () => {
      while (offset < payload.length) {
        const chunk = payload.slice(offset, offset + chunkSize);
        offset += chunkSize;
        if (!request.write(chunk)) return request.once("drain", pump);
      }
      request.end();
    };
    request.on("error", reject);
    pump();
  });
}

/** A request/response pair for cases a real socket cannot produce (e.g. remoteAddress). */
function fakeExchange({ url = "/__orso", method = "POST", headers = {}, remoteAddress = "10.1.2.3" } = {}) {
  const req = {
    url,
    method,
    headers,
    socket: { remoteAddress },
    on() {},
    resume() {},
  };
  const res = {
    statusCode: 0,
    chunks: [],
    headers: {},
    setHeader(name, value) {
      this.headers[name.toLowerCase()] = value;
    },
    getHeader(name) {
      return this.headers[name.toLowerCase()];
    },
    end(chunk) {
      if (chunk !== undefined) this.chunks.push(String(chunk));
      this.ended = true;
    },
  };
  return { req, res };
}

test("endpoint only handles the exact /__orso path and calls next() otherwise", async () => {
  for (const path of ["/", "/other", "/__orso/", "/__orsoo"]) {
    const response = await post({ op: "validate", collection: "blog", fields: {} }, { path });
    assert.equal(response.status, 418, path);
    assert.equal(await response.text(), "next", path);
  }

  // The match is on pathname, so a query string does not change the route.
  const queried = await post(
    { op: "validate", collection: "blog", fields: { title: "T", description: "D", pubDate: "2026-10-11" } },
    { path: "/__orso?editor=1" },
  );
  assert.equal(queried.status, 200);

  const { req, res } = fakeExchange({ url: "/__orso/", method: "POST" });
  let nextCalls = 0;
  endpoint(req, res, () => nextCalls++);
  assert.equal(nextCalls, 1);
  assert.equal(res.ended, undefined);
});

test("endpoint rejects methods other than POST with 405", async () => {
  for (const method of ["GET", "PUT", "DELETE", "PATCH"]) {
    const response = await post(undefined, { method });
    assert.equal(response.status, 405, method);
  }
});

test("endpoint requires an application/json content type", async () => {
  const response = await post({ op: "validate", collection: "blog", fields: {} }, { contentType: "text/plain" });
  assert.equal(response.status, 400);
  assert.deepEqual(await response.json(), {
    error: "bad_request",
    message: "Content-Type must be application/json",
  });
  assert.match(response.headers.get("content-type"), /^application\/json/);
  assert.equal(response.headers.get("access-control-allow-origin"), null);

  const parameters = await post(
    { op: "validate", collection: "blog", fields: { title: "T", description: "D", pubDate: "2026-10-11" } },
    { contentType: "application/json; charset=utf-8" },
  );
  assert.equal(parameters.status, 200);
});

test("endpoint rejects missing, wrong and wrong-length tokens with 401", async () => {
  const body = { op: "validate", collection: "blog", fields: {} };
  for (const token of [undefined, "", "wrong-token-000", TOKEN + "-longer", "é" + TOKEN.slice(1)]) {
    const response = await fetch(origin + "/__orso", {
      method: "POST",
      headers: token === undefined ? { "content-type": "application/json" } : { "content-type": "application/json", "x-orso-content-token": token },
      body: JSON.stringify(body),
    });
    assert.deepEqual(await response.json(), { error: "unauthorized" });
    assert.equal(response.status, 401);
  }
});

test("endpoint rejects non-loopback socket addresses with 403", () => {
  for (const remoteAddress of ["10.1.2.3", "192.168.0.9", "2001:db8::1", "::ffff:10.1.2.3"]) {
    const { req, res } = fakeExchange({ remoteAddress });
    let nextCalls = 0;
    endpoint(req, res, () => nextCalls++);
    assert.equal(res.statusCode, 403, remoteAddress);
    assert.equal(nextCalls, 0, remoteAddress);
    assert.deepEqual(JSON.parse(res.chunks.join("")), { error: "forbidden", message: "Requests must come from loopback" });
  }

  for (const remoteAddress of ["127.0.0.1", "::1", "::ffff:127.0.0.1"]) {
    const { req, res } = fakeExchange({ remoteAddress });
    endpoint(req, res, () => {
      throw new Error("loopback must not call next()");
    });
    assert.equal(res.statusCode, 400, remoteAddress);
  }
});

test("endpoint rejects a Content-Length over 2 MiB with 413 before reading the body", async () => {
  const status = await new Promise((resolve, reject) => {
    const request = http.request(
      {
        host: "127.0.0.1",
        port: server.address().port,
        path: "/__orso",
        method: "POST",
        headers: {
          "content-type": "application/json",
          "x-orso-content-token": TOKEN,
          "content-length": String(MAX_BODY + 1),
        },
      },
      (response) => {
        response.resume();
        request.destroy();
        resolve(response.statusCode);
      },
    );
    request.on("error", reject);
    request.flushHeaders();
  });
  assert.equal(status, 413);
  assert.equal(lastRequestHeaders["content-length"], String(MAX_BODY + 1));
});

test("endpoint stops reading a streamed body over 2 MiB with 413", async () => {
  const response = await streamPost(
    JSON.stringify({ op: "validate", collection: "blog", fields: { title: "x".repeat(MAX_BODY) } }),
  );

  // The server saw a chunked upload, so this exercises the streaming cap rather
  // than the declared Content-Length check covered above.
  assert.equal(lastRequestHeaders["content-length"], undefined);
  assert.equal(lastRequestHeaders["transfer-encoding"], "chunked");

  assert.equal(response.status, 413);
  assert.deepEqual(response.body, { error: "payload_too_large", message: "Request body exceeds 2 MiB" });
  // Writing continued after the 413 without the server resetting the connection.
  assert.notEqual(response.headers.connection, "close");
});

test("endpoint validate returns parsed and defaulted fields with no errors", async () => {
  const { status, body, headers } = await json(
    await post({
      op: "validate",
      collection: "blog",
      fields: { title: "Hello", description: "World", pubDate: "2026-10-11" },
    }),
  );
  assert.equal(status, 200);
  assert.deepEqual(body, {
    valid: true,
    fields: { title: "Hello", description: "World", pubDate: "2026-10-11", tags: [], relatedPosts: [] },
    errors: [],
  });
  assert.match(headers.get("content-type"), /^application\/json/);
  assert.equal(headers.get("access-control-allow-origin"), null);
});

test("endpoint validate returns valid:false with the original fields and issue paths", async () => {
  const fields = { description: "World", pubDate: "2026-02-30" };
  const { status, body } = await json(await post({ op: "validate", collection: "blog", fields }));
  assert.equal(status, 200);
  assert.equal(body.valid, false);
  assert.deepEqual(body.fields, fields);
  assert.deepEqual(body.errors.map(({ path }) => path).sort(), [["pubDate"], ["title"]]);
  assert.ok(body.errors.every(({ message }) => typeof message === "string" && message.length > 0));
});

test("endpoint returns 404 for a collection absent from the manifest", async () => {
  const { status, body } = await json(await post({ op: "validate", collection: "missing", fields: {} }));
  assert.equal(status, 404);
  assert.equal(body.error, "unsupported_collection");
  assert.match(body.message, /missing/);
});

test("endpoint returns 400 with the manifest reason for an unsupported collection", async () => {
  for (const op of ["validate", "patch"]) {
    const { status, body } = await json(await post({ op, collection: "readOnly", source: "---\n---\n", fields: {} }));
    assert.equal(status, 400, op);
    assert.equal(body.error, "unsupported_collection");
    assert.match(body.message, /plain glob\(\)/);
    assert.match(body.message, /readOnly/);
  }
});

test("endpoint patch returns the patched source", async () => {
  const source = "---\ntitle: Old\ndescription: Kept\npubDate: 2026-10-11\n---\nBody\n";
  const { status, body } = await json(await post({ op: "patch", collection: "blog", source, fields: { title: "New" } }));
  assert.equal(status, 200);
  assert.deepEqual(body, { source: source.replace("title: Old", "title: New") });

  const bodyEdit = await json(await post({ op: "patch", collection: "blog", source, fields: {}, body: "Changed\n" }));
  assert.equal(bodyEdit.status, 200);
  assert.deepEqual(bodyEdit.body, { source: source.slice(0, source.indexOf("Body")) + "Changed\n" });
});

test("endpoint patch maps schema failures to 422 invalid_schema with packaged issues", async () => {
  const source = "---\ntitle: Old\ndescription: Kept\npubDate: 2026-10-11\n---\nBody\n";
  const { status, body } = await json(
    await post({ op: "patch", collection: "blog", source, fields: { pubDate: "2026-02-30" } }),
  );
  assert.equal(status, 422);
  assert.deepEqual(body, {
    error: "invalid_schema",
    issues: [{ path: ["pubDate"], message: "Use a valid calendar date" }],
  });
});

test("endpoint patch maps unsafe sources, overlapping unset and malformed requests to 400", async () => {
  const source = "---\ntitle: Old\ndescription: Kept\npubDate: 2026-10-11\n---\nBody\n";
  for (const request of [
    { op: "patch", collection: "blog", fields: {} },
    { op: "patch", collection: "blog", source: 42, fields: {} },
    { op: "patch", collection: "blog", source, fields: [] },
    { op: "patch", collection: "blog", source, fields: {}, unset: "title" },
    { op: "patch", collection: "blog", source, fields: { title: "New" }, unset: ["title"] },
    { op: "patch", collection: "blog", source: "no frontmatter", fields: {} },
    { op: "patch", collection: "blog", source: "---\n- one\n---\n", fields: {} },
  ]) {
    const { status, body } = await json(await post(request));
    assert.equal(status, 400, JSON.stringify(request));
    assert.equal(body.error, "bad_request");
    assert.ok(typeof body.message === "string" && body.message.length > 0);
  }
});

test("endpoint rejects malformed JSON and unknown operations with 400", async () => {
  for (const request of ["{", "[]", "null", JSON.stringify({ op: "delete", collection: "blog" }), JSON.stringify({ op: "validate", fields: {} })]) {
    const response = await post(request);
    assert.equal(response.status, 400, request);
    assert.equal((await response.json()).error, "bad_request");
  }
});

test("endpoint returns a generic 500 and logs only server-side detail", async () => {
  const logged = [];
  const original = console.error;
  // Mirror console.error, which inspects object arguments via util.format.
  console.error = (...args) => logged.push(format(...args));
  try {
    const response = await post({
      op: "validate",
      collection: "broken",
      fields: { marker: "body-marker-should-not-be-logged" },
    });
    const { status, body } = await json(response);
    assert.equal(status, 500);
    assert.equal(body.error, "internal");
    assert.ok(typeof body.message === "string" && body.message.length > 0);
    assert.ok(!/body-marker-should-not-be-logged/.test(JSON.stringify(body)));

    // A custom property on the error can carry body-derived values; only
    // name/message may be logged.
    const leaky = await post({ op: "validate", collection: "leaky", fields: { marker: "body-detail-marker" } });
    assert.equal(leaky.status, 500);
    assert.ok(!/body-detail-marker/.test(JSON.stringify(await leaky.json())));
  } finally {
    console.error = original;
  }
  const text = logged.join("\n");
  assert.match(text, /Error: schema exploded/);
  assert.match(text, /Error: schema rejected input/);
  assert.ok(!text.includes(TOKEN));
  assert.ok(!text.includes("body-marker-should-not-be-logged"));
  assert.ok(!text.includes("body-detail-marker"));
  assert.ok(!/body-marker-should-not-be-logged|body-detail-marker|\[Object|details/.test(text));
});

test("endpoint fails closed when the server has no token", async () => {
  const { req, res } = fakeExchange({ remoteAddress: "127.0.0.1", headers: { "content-type": "application/json", "x-orso-content-token": "anything" } });
  createEndpoint({ token: "", lookup })(req, res, () => assert.fail("must not call next()"));
  assert.equal(res.statusCode, 401);
});
