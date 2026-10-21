import { timingSafeEqual } from "node:crypto";

import { PatchError, SchemaIssuesError, patchSource } from "./patch.js";

/** Entries are capped at 1 MiB; a patch carries the entry plus its update. */
const MAX_BODY = 2 * 1024 * 1024;

/** A request body that exceeded {@link MAX_BODY} while streaming. */
class BodyTooLargeError extends Error {
  constructor() {
    super("Request body exceeds 2 MiB");
    this.name = "BodyTooLargeError";
    this.code = "body_too_large";
  }
}

/** Malformed request shape; reported as 400 before any schema work happens. */
class BadRequestError extends Error {
  constructor(message) {
    super(message);
    this.name = "BadRequestError";
  }
}

/** A complete response decided during lookup, such as 404 or unsupported 400. */
class HttpError extends Error {
  constructor(status, body) {
    super(body.message ?? body.error);
    this.name = "HttpError";
    this.status = status;
    this.body = body;
  }
}

/**
 * Loopback detection for `req.socket.remoteAddress`: IPv4 `127.0.0.0/8`, IPv6
 * `::1`, and the IPv4-mapped forms Node reports for dual-stack sockets. An
 * absent address (for example a Unix socket) is not a remote client.
 */
function isLoopback(address) {
  if (address === undefined || address === null) return true;
  if (typeof address !== "string") return false;
  const value = address.replace(/^::ffff:/i, "");
  if (value === "::1") return true;
  const octets = value.split(".");
  return octets.length === 4 && octets.every((part) => /^\d{1,3}$/.test(part) && Number(part) <= 255) && octets[0] === "127";
}

/** Constant-time token comparison that never hashes or logs either value. */
function tokenMatches(expected, provided) {
  if (typeof expected !== "string" || expected === "") return false;
  if (typeof provided !== "string") return false;
  const expectedBytes = Buffer.from(expected);
  const providedBytes = Buffer.from(provided);
  if (providedBytes.length !== expectedBytes.length) return false;
  return timingSafeEqual(providedBytes, expectedBytes);
}

/** Write a JSON response; never adds CORS headers. */
function send(res, status, body, headers = {}) {
  const payload = JSON.stringify(body);
  res.statusCode = status;
  res.setHeader("content-type", "application/json; charset=utf-8");
  res.setHeader("content-length", Buffer.byteLength(payload));
  res.setHeader("cache-control", "no-store");
  for (const [name, value] of Object.entries(headers)) res.setHeader(name, value);
  res.end(payload);
}

/**
 * Safe server-side detail: `Error#message` only. Inspecting a raw error object
 * could print request-derived values held on custom properties (for example a
 * schema error carrying the rejected fields).
 */
function describeError(error) {
  if (error instanceof Error) return `${error.name}: ${error.message}`;
  return typeof error === "string" ? error : "unknown error";
}

/**
 * Discard an unread request body so the connection stays usable after an early
 * rejection; chunks are dropped rather than buffered.
 */
function drain(req) {
  req.on?.("error", () => {});
  req.resume?.();
}

/**
 * Read the request body up to {@link MAX_BODY}. When the cap is crossed the
 * promise rejects immediately (so the caller can answer 413 without waiting)
 * and the rest of the stream is drained rather than buffered.
 */
function readBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    let size = 0;
    let settled = false;
    const fail = (error) => {
      if (settled) return;
      settled = true;
      req.off?.("data", onData);
      req.resume?.();
      reject(error);
    };
    const onData = (chunk) => {
      size += chunk.length;
      if (size > MAX_BODY) return fail(new BodyTooLargeError());
      chunks.push(chunk);
    };
    req.on("data", onData);
    req.on("end", () => {
      if (settled) return;
      settled = true;
      resolve(Buffer.concat(chunks).toString("utf8"));
    });
    req.on("error", (error) => fail(error));
    req.on("aborted", () => fail(new BadRequestError("Request body was aborted")));
  });
}

const issuesOf = (error) => (Array.isArray(error?.issues) ? error.issues : []).map(({ path, message }) => ({ path, message }));

function requireEntry(collection, lookup) {
  const entry = lookup(collection);
  if (!entry) {
    throw new HttpError(404, {
      error: "unsupported_collection",
      message: `Collection "${collection}" is not in the current manifest`,
    });
  }
  if (entry.supportedForEditing !== true) {
    const reason = typeof entry.reason === "string" && entry.reason !== "" ? `: ${entry.reason}` : "";
    throw new HttpError(400, {
      error: "unsupported_collection",
      message: `Collection "${collection}" is not supported for editing${reason}`,
    });
  }
  return entry;
}

function requireSchema(entry, collection) {
  if (!entry.schema || typeof entry.schema.safeParse !== "function") {
    throw new BadRequestError(`Collection "${collection}" has no editable schema`);
  }
  return entry.schema;
}

/** A complete candidate; defaults from the schema are returned on success. */
function validate(request, entry) {
  const { fields } = request;
  if (!fields || typeof fields !== "object" || Array.isArray(fields)) throw new BadRequestError("fields must be an object");
  const result = requireSchema(entry, request.collection).safeParse(fields);
  if (result.success) return { status: 200, body: { valid: true, fields: result.data, errors: [] } };
  return { status: 200, body: { valid: false, fields, errors: issuesOf(result.error) } };
}

/** A partial top-level update applied to the source without reformatting it. */
function patch(request, entry) {
  if (typeof request.source !== "string") throw new BadRequestError("source must be a string");
  const source = patchSource(
    { source: request.source, fields: request.fields, unset: request.unset, body: request.body },
    { kind: entry.kind, schema: requireSchema(entry, request.collection) },
  );
  return { status: 200, body: { source } };
}

function operation(request, lookup) {
  if (!request || typeof request !== "object" || Array.isArray(request)) throw new BadRequestError("Request body must be a JSON object");
  if (typeof request.collection !== "string" || request.collection === "") throw new BadRequestError("collection must be a non-empty string");
  if (request.op !== "validate" && request.op !== "patch") throw new BadRequestError('op must be "validate" or "patch"');
  const entry = requireEntry(request.collection, lookup);
  return request.op === "validate" ? validate(request, entry) : patch(request, entry);
}

/**
 * Create the dev-only `POST /__orso` middleware.
 *
 * `lookup(name)` resolves a collection from the current manifest and returns
 * `{ supportedForEditing, kind, schema, reason }`, or `undefined` when the
 * collection is absent. The handler is an exact-path middleware: every other
 * request goes to `next()`. It refuses non-loopback clients, non-JSON content
 * types, wrong tokens, oversized bodies and non-POST methods, sends no CORS
 * headers and never logs the token or the request body.
 */
export function createEndpoint({ token, lookup } = {}) {
  if (typeof lookup !== "function") throw new TypeError("createEndpoint: lookup must be a function");

  return async function orsoEndpoint(req, res, next) {
    let pathname;
    try {
      pathname = new URL(req.url ?? "/", "http://localhost").pathname;
    } catch {
      pathname = null;
    }
    if (pathname !== "/__orso") return next();

    if (req.method !== "POST") return send(res, 405, { error: "method_not_allowed", message: "Only POST is supported" });
    if (!isLoopback(req.socket?.remoteAddress)) {
      return send(res, 403, { error: "forbidden", message: "Requests must come from loopback" });
    }

    const contentType = req.headers?.["content-type"];
    if (typeof contentType !== "string" || contentType.split(";")[0].trim().toLowerCase() !== "application/json") {
      return send(res, 400, { error: "bad_request", message: "Content-Type must be application/json" });
    }
    if (!tokenMatches(token, req.headers?.["x-orso-content-token"])) return send(res, 401, { error: "unauthorized" });

    const declared = Number(req.headers?.["content-length"]);
    if (Number.isFinite(declared) && declared > MAX_BODY) {
      send(res, 413, { error: "payload_too_large", message: "Request body exceeds 2 MiB" });
      return drain(req);
    }

    let raw;
    try {
      raw = await readBody(req);
    } catch (error) {
      if (error?.code === "body_too_large") {
        send(res, 413, { error: "payload_too_large", message: error.message });
        return drain(req);
      }
      console.error(`orso content endpoint could not read the request body: ${describeError(error)}`);
      return send(res, 400, { error: "bad_request", message: "Could not read the request body" });
    }

    let request;
    try {
      request = JSON.parse(raw);
    } catch {
      return send(res, 400, { error: "bad_request", message: "Request body must be valid JSON" });
    }

    try {
      const { status, body } = operation(request, lookup);
      return send(res, status, body);
    } catch (error) {
      if (error instanceof SchemaIssuesError) return send(res, 422, { error: "invalid_schema", issues: issuesOf(error) });
      if (error instanceof PatchError || error instanceof BadRequestError) {
        return send(res, 400, { error: "bad_request", message: error.message });
      }
      if (error instanceof HttpError) return send(res, error.status, error.body);
      console.error(`orso content endpoint failed: ${describeError(error)}`);
      return send(res, 500, { error: "internal", message: "The content endpoint failed" });
    }
  };
}
