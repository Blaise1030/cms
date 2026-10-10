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

export type GithubStub = {
  baseUrl: string;
  /** Bodies of every `POST /repos/{o}/{r}/pulls` the daemon sent. */
  pullsPosts: () => PullsPost[];
  /** Force pulls endpoints to return this HTTP status (null = normal). */
  setPullsStatus: (status: number | null) => void;
  /** `GET /repos/{o}/{r}/branches/main/protection`: 200 when true (default), 404 when false. */
  setMainProtected: (isProtected: boolean) => void;
  /** Number of branch-protection lookups the daemon made. */
  protectionGets: () => number;
  /** `GET /repos/{o}/{r}/rules/branches/main`: a `pull_request` rule when true, else `[]`. */
  setMainRulesetRequiresPr: (requiresPr: boolean) => void;
  close: () => Promise<void>;
};

type StoredPr = {
  number: number;
  title: string;
  head: string;
  html_url: string;
  draft: boolean;
};

/** Minimal GitHub API stub: device flow + `/user` + repos + draft pulls. */
export function startGithubStub(clientId = "cid"): Promise<GithubStub> {
  let pollCount = 0;
  let pullsStatus: number | null = null;
  let mainProtected = true;
  let protectionGetCount = 0;
  let rulesetRequiresPr = false;
  const pullsPosts: PullsPost[] = [];
  const prs: StoredPr[] = [];
  let nextPr = 1;

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

    if (
      req.method === "GET" &&
      /^\/repos\/[^/]+\/[^/]+\/branches\/main\/protection$/.test(url.pathname)
    ) {
      if (auth !== `Bearer ${STUB_TOKEN}`) {
        res.writeHead(401, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ message: "Bad credentials" }));
        return;
      }
      protectionGetCount += 1;
      res.writeHead(mainProtected ? 200 : 404, { "Content-Type": "application/json" });
      res.end(
        JSON.stringify(
          mainProtected
            ? { required_pull_request_reviews: { required_approving_review_count: 1 } }
            : { message: "Branch not protected" },
        ),
      );
      return;
    }

    if (
      req.method === "GET" &&
      /^\/repos\/[^/]+\/[^/]+\/rules\/branches\/main$/.test(url.pathname)
    ) {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(
        JSON.stringify(
          rulesetRequiresPr
            ? [{ type: "pull_request", parameters: { required_approving_review_count: 1 } }]
            : [],
        ),
      );
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
        const matching = head ? prs.filter((p) => p.head === head) : prs;
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify(matching));
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

    res.writeHead(404).end();
  });

  return new Promise((resolve, reject) => {
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address() as AddressInfo;
      resolve({
        baseUrl: `http://127.0.0.1:${port}`,
        pullsPosts: () => pullsPosts.slice(),
        setPullsStatus: (status) => {
          pullsStatus = status;
        },
        setMainProtected: (isProtected) => {
          mainProtected = isProtected;
        },
        protectionGets: () => protectionGetCount,
        setMainRulesetRequiresPr: (requiresPr) => {
          rulesetRequiresPr = requiresPr;
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
