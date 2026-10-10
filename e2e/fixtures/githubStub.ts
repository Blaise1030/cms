import http from "node:http";
import type { AddressInfo } from "node:net";

export const STUB_TOKEN = "tok";
export const STUB_USER = {
  login: "ada",
  id: 42,
  name: "Ada Lovelace",
  avatar_url: "https://avatars.example/ada.png",
};

export type PullsPost = {
  path: string;
  body: {
    title?: string;
    head?: string;
    base?: string;
    draft?: boolean;
    body?: string;
  };
};

/** Head SHA a stub PR reports when `headSha` can't resolve its branch. */
export const STUB_HEAD_SHA = "5b1e5b1e5b1e5b1e5b1e5b1e5b1e5b1e5b1e5b1e";

export type CheckRun = { status: string; conclusion: string | null };
export type Review = { user: { login: string }; state: string };

/** Overrides applied to every stub PR (status endpoints read them). */
export type PrStatusPatch = {
  draft?: boolean;
  state?: "open" | "closed";
  merged?: boolean;
  checkRuns?: CheckRun[];
  reviews?: Review[];
  /** A successful deployment for the head SHA (null = none). */
  deployment?: { environment_url: string } | null;
};

export type GithubStub = {
  baseUrl: string;
  /** Bodies of every `POST /repos/{o}/{r}/pulls` the daemon sent. */
  pullsPosts: () => PullsPost[];
  /** Force pulls endpoints to return this HTTP status (null = normal). */
  setPullsStatus: (status: number | null) => void;
  /** Change PR state / checks / reviews / deployment served by the status endpoints. */
  setPrStatus: (patch: PrStatusPatch) => void;
  /** PR node ids of every GraphQL `markPullRequestReadyForReview` mutation received. */
  readyForReviewCalls: () => string[];
  /** Force `POST /graphql` to return this HTTP status (null = normal). */
  setGraphqlStatus: (status: number | null) => void;
  close: () => Promise<void>;
};

type StoredPr = {
  number: number;
  title: string;
  head: string;
  html_url: string;
  draft: boolean;
};

export type GithubStubOptions = {
  /** Real head commit of a PR's branch (e.g. read from the bare remote); null = unknown. */
  headSha?: (ref: string) => string | null;
};

/** GraphQL node id GitHub would give PR `number`. */
const nodeId = (number: number) => `PR_kwStub${number}`;

/** Minimal GitHub API stub: device flow + `/user` + repos + draft pulls + GraphQL
 *  `markPullRequestReadyForReview`. */
export function startGithubStub(
  clientId = "cid",
  opts: GithubStubOptions = {},
): Promise<GithubStub> {
  let pollCount = 0;
  let pullsStatus: number | null = null;
  let graphqlStatus: number | null = null;
  const readyCalls: string[] = [];
  const pullsPosts: PullsPost[] = [];
  const prs: StoredPr[] = [];
  let nextPr = 1;
  const status: Required<Omit<PrStatusPatch, "draft">> & { draft: boolean | null } = {
    draft: null,
    state: "open",
    merged: false,
    checkRuns: [],
    reviews: [],
    deployment: null,
  };
  const pullJson = (p: StoredPr) => {
    const headRef = p.head.split(":").slice(1).join(":");
    return {
      number: p.number,
      node_id: nodeId(p.number),
      title: p.title,
      html_url: p.html_url,
      draft: status.draft ?? p.draft,
      state: status.state,
      merged_at: status.merged ? "2026-10-04T00:00:00Z" : null,
      head: { ref: headRef, sha: opts.headSha?.(headRef) ?? STUB_HEAD_SHA },
    };
  };
  const sendJson = (res: http.ServerResponse, code: number, body: unknown) => {
    res.writeHead(code, { "Content-Type": "application/json" });
    res.end(JSON.stringify(body));
  };

  const server = http.createServer((req, res) => {
    const url = new URL(req.url ?? "/", "http://localhost");
    const auth = req.headers.authorization;

    if (req.method === "POST" && url.pathname === "/login/device/code") {
      let body = "";
      req.on("data", (c) => {
        body += c;
      });
      req.on("end", () => {
        if (!body.includes(`client_id=${clientId}`)) {
          res.writeHead(400).end();
          return;
        }
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(
          JSON.stringify({
            device_code: "dev123",
            user_code: "ABCD-1234",
            verification_uri: "https://github.com/login/device",
            expires_in: 900,
            interval: 1,
          }),
        );
      });
      return;
    }

    if (req.method === "POST" && url.pathname === "/login/oauth/access_token") {
      let body = "";
      req.on("data", (c) => {
        body += c;
      });
      req.on("end", () => {
        if (!body.includes("device_code=dev123")) {
          res.writeHead(400).end();
          return;
        }
        pollCount += 1;
        res.writeHead(200, { "Content-Type": "application/json" });
        if (pollCount === 1) {
          res.end(JSON.stringify({ error: "authorization_pending" }));
        } else {
          res.end(JSON.stringify({ access_token: STUB_TOKEN, token_type: "bearer" }));
        }
      });
      return;
    }

    if (req.method === "GET" && url.pathname === "/user") {
      if (auth !== `Bearer ${STUB_TOKEN}`) {
        res.writeHead(401, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ message: "Bad credentials" }));
        return;
      }
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify(STUB_USER));
      return;
    }

    if (req.method === "GET" && url.pathname === "/user/repos") {
      if (auth !== `Bearer ${STUB_TOKEN}`) {
        res.writeHead(401, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ message: "Bad credentials" }));
        return;
      }
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(
        JSON.stringify([
          {
            full_name: "acme/site",
            private: true,
            default_branch: "main",
            permissions: { push: true },
          },
        ]),
      );
      return;
    }

    if (req.method === "POST" && url.pathname === "/graphql") {
      if (auth !== `Bearer ${STUB_TOKEN}`) {
        sendJson(res, 401, { message: "Bad credentials" });
        return;
      }
      if (graphqlStatus !== null) {
        sendJson(res, graphqlStatus, { message: graphqlStatus === 403 ? "Resource not accessible" : "error" });
        return;
      }
      let raw = "";
      req.on("data", (c) => {
        raw += c;
      });
      req.on("end", () => {
        let body: { query?: string; variables?: Record<string, unknown> } = {};
        try {
          body = JSON.parse(raw) as typeof body;
        } catch {
          res.writeHead(400).end();
          return;
        }
        if (!body.query?.includes("markPullRequestReadyForReview")) {
          sendJson(res, 200, { errors: [{ message: "unsupported query" }] });
          return;
        }
        const id = Object.values(body.variables ?? {}).find((v) => typeof v === "string") as
          | string
          | undefined;
        const pr = prs.find((p) => nodeId(p.number) === id);
        if (!id || !pr) {
          sendJson(res, 200, {
            data: { markPullRequestReadyForReview: null },
            errors: [{ type: "NOT_FOUND", message: `Could not resolve to a node with the global id of '${id}'` }],
          });
          return;
        }
        readyCalls.push(id);
        pr.draft = false;
        status.draft = null;
        sendJson(res, 200, {
          data: { markPullRequestReadyForReview: { pullRequest: { isDraft: false } } },
        });
      });
      return;
    }

    const pullsMatch = url.pathname.match(/^\/repos\/([^/]+)\/([^/]+)\/pulls$/);
    if (pullsMatch) {
      if (auth !== `Bearer ${STUB_TOKEN}`) {
        res.writeHead(401, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ message: "Bad credentials" }));
        return;
      }
      if (pullsStatus !== null) {
        res.writeHead(pullsStatus, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ message: pullsStatus === 403 ? "Resource not accessible" : "error" }));
        return;
      }

      if (req.method === "GET") {
        const head = url.searchParams.get("head");
        const want = url.searchParams.get("state") ?? "open";
        const matching = (head ? prs.filter((p) => p.head === head) : prs).filter(
          () => want === "all" || want === status.state,
        );
        sendJson(res, 200, matching.map(pullJson));
        return;
      }

      if (req.method === "POST") {
        let raw = "";
        req.on("data", (c) => {
          raw += c;
        });
        req.on("end", () => {
          let body: PullsPost["body"] = {};
          try {
            body = JSON.parse(raw) as PullsPost["body"];
          } catch {
            res.writeHead(400).end();
            return;
          }
          pullsPosts.push({ path: url.pathname, body });
          const number = nextPr++;
          const owner = pullsMatch[1]!;
          const repo = pullsMatch[2]!;
          const headRef = typeof body.head === "string" ? body.head : "";
          const stored: StoredPr = {
            number,
            title: typeof body.title === "string" ? body.title : "",
            head: `${owner}:${headRef}`,
            html_url: `https://github.com/${owner}/${repo}/pull/${number}`,
            draft: body.draft === true,
          };
          prs.push(stored);
          res.writeHead(201, { "Content-Type": "application/json" });
          res.end(
            JSON.stringify({
              number: stored.number,
              node_id: nodeId(stored.number),
              title: stored.title,
              html_url: stored.html_url,
              draft: stored.draft,
              head: { ref: headRef },
            }),
          );
        });
        return;
      }
    }

    const repoGet =
      req.method === "GET" && url.pathname.match(/^\/repos\/[^/]+\/[^/]+(\/.*)$/);
    if (repoGet) {
      if (auth !== `Bearer ${STUB_TOKEN}`) {
        sendJson(res, 401, { message: "Bad credentials" });
        return;
      }
      const rest = repoGet[1]!;
      if (/^\/pulls\/\d+\/reviews$/.test(rest)) {
        sendJson(res, 200, status.reviews);
        return;
      }
      // Checks and deployments are served for whatever head SHA is asked about.
      if (/^\/commits\/[0-9a-f]{40}\/check-runs$/.test(rest)) {
        sendJson(res, 200, { total_count: status.checkRuns.length, check_runs: status.checkRuns });
        return;
      }
      if (rest === "/deployments") {
        const sha = url.searchParams.get("sha");
        const list = status.deployment
          ? [{ id: 1, sha: sha ?? STUB_HEAD_SHA, environment: "preview" }]
          : [];
        sendJson(res, 200, list);
        return;
      }
      if (rest === "/deployments/1/statuses") {
        sendJson(
          res,
          200,
          status.deployment
            ? [{ state: "success", environment_url: status.deployment.environment_url }]
            : [],
        );
        return;
      }
    }

    res.writeHead(404).end();
  });

  return new Promise((resolve, reject) => {
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address() as AddressInfo;
      resolve({
        baseUrl: `http://127.0.0.1:${port}`,
        pullsPosts: () => pullsPosts.slice(),
        setPullsStatus: (code) => {
          pullsStatus = code;
        },
        setPrStatus: (patch) => {
          Object.assign(status, patch);
        },
        readyForReviewCalls: () => readyCalls.slice(),
        setGraphqlStatus: (code) => {
          graphqlStatus = code;
        },
        close: () =>
          new Promise((done, err) => {
            server.close((e) => (e ? err(e) : done()));
          }),
      });
    });
    server.on("error", reject);
  });
}
