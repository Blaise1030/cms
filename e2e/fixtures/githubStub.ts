import http from "node:http";
import type { AddressInfo } from "node:net";

export const STUB_TOKEN = "tok";
export const STUB_USER = {
  login: "ada",
  id: 42,
  name: "Ada Lovelace",
  avatar_url: "https://avatars.example/ada.png",
};

export type GithubStub = {
  baseUrl: string;
  close: () => Promise<void>;
};

/** Minimal GitHub API stub: device flow + `/user` + one repo for clone smoke tests. */
export function startGithubStub(clientId = "cid"): Promise<GithubStub> {
  let pollCount = 0;
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

    res.writeHead(404).end();
  });

  return new Promise((resolve, reject) => {
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address() as AddressInfo;
      resolve({
        baseUrl: `http://127.0.0.1:${port}`,
        close: () =>
          new Promise((done, err) => {
            server.close((e) => (e ? err(e) : done()));
          }),
      });
    });
    server.on("error", reject);
  });
}
