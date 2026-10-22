/**
 * A stand-in for `astro dev` with the orso integration: serves `POST /__orso`
 * for the fixture `blog`/`helpDocs` schemas, without reading any project config.
 *
 * Usage: `ORSO_CONTENT_TOKEN=<token> node fake-devserver.mjs <port>`
 */
import http from "node:http";

import { z } from "astro/zod";

import { createEndpoint } from "../../src/endpoint.js";
import { createCollectionSchemas } from "./schemas.mjs";

/** Leak guard: a supervisor that never stops us must not leave a server behind. */
const LIFETIME_MS = 60_000;

const token = process.env.ORSO_CONTENT_TOKEN;
if (!token) {
  console.error("fake-devserver: ORSO_CONTENT_TOKEN is not set");
  process.exit(1);
}
const port = Number(process.argv[2]);
if (!Number.isInteger(port) || port < 0 || port > 65535) {
  console.error(`fake-devserver: invalid port ${JSON.stringify(process.argv[2])}`);
  process.exit(1);
}

const schemas = createCollectionSchemas({ z });
const collections = {
  blog: { supportedForEditing: true, kind: "markdown", schema: schemas.blog, reason: null },
  helpDocs: { supportedForEditing: true, kind: "markdown", schema: schemas.helpDocs, reason: null },
};
const endpoint = createEndpoint({ token, lookup: (name) => collections[name] });

const server = http.createServer((req, res) => {
  endpoint(req, res, () => {
    res.statusCode = 404;
    res.end();
  });
});

/**
 * The port the supervisor chose can be taken by a parallel test before we bind;
 * like Astro, fall back to another port and announce the one we got.
 */
function listen(port) {
  server.once("error", (error) => {
    if (error.code === "EADDRINUSE" && port !== 0) return listen(0);
    console.error(`fake-devserver: ${error.message}`);
    process.exit(1);
  });
  server.listen(port, "127.0.0.1");
}

server.once("listening", () => console.log(`  Local http://127.0.0.1:${server.address().port}/`));
listen(port);
setTimeout(() => process.exit(0), LIFETIME_MS).unref();
