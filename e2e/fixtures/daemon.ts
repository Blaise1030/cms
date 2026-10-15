import { execFileSync, spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import readline from "node:readline";
import { fileURLToPath } from "node:url";

import { createBareRemote, DEFAULT_REPO, fileGitUrl } from "./bareRemote";
import { startGithubStub } from "./githubStub";

export const E2E_TOKEN = "a".repeat(64);

export type WorkbenchEnv = {
  origin: string;
  token: string;
  projectId: string;
  projectDir: string;
  bareRemote: string;
  tmp: string;
  /** GitHub API stub — inspect pulls POSTs / force error statuses. */
  github: {
    pullsPosts: () => import("./githubStub").PullsPost[];
    setPullsStatus: (status: number | null) => void;
    setPrStatus: (patch: import("./githubStub").PrStatusPatch) => void;
    /** Stub base URL (also the daemon's GitHub web URL). */
    baseUrl: string;
    setMainProtected: (isProtected: boolean) => void;
    protectionGets: () => number;
    setMainRulesetRequiresPr: (requiresPr: boolean) => void;
    readyForReviewCalls: () => string[];
    setGraphqlStatus: (status: number | null) => void;
  };
  stop: () => Promise<void>;
};

function repoRoot(): string {
  return path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
}

function fakeCliPath(root = repoRoot()): string {
  return path.join(root, "tests/fixtures/fake_cli.sh");
}

/** `http(s)://host` → `ws(s)://host/api/terminal/{project}/ws[?t=token]`. */
function buildTerminalUrl(
  origin: string,
  token: string,
  projectId: string,
): string {
  const url = new URL(`/api/terminal/${encodeURIComponent(projectId)}/ws`, origin);
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  if (token) url.searchParams.set("t", token);
  return url.toString();
}

export async function api(
  origin: string,
  token: string,
  method: string,
  uri: string,
  body?: unknown,
): Promise<Response> {
  return fetch(`${origin}${uri}`, {
    method,
    headers: {
      Authorization: `Bearer ${token}`,
      ...(body ? { "Content-Type": "application/json" } : {}),
    },
    body: body ? JSON.stringify(body) : undefined,
  });
}

/** Start a change on the daemon (the UI's New change menu asks the agent instead). */
export async function createChange(
  env: Pick<WorkbenchEnv, "origin" | "token" | "projectId">,
  title: string,
): Promise<void> {
  const res = await api(env.origin, env.token, "POST", "/api/changes", { project: env.projectId, title });
  if (!res.ok) throw new Error(`create change failed: ${res.status} ${await res.text()}`);
}

/**
 * Drive a fake CLI turn over the terminal WebSocket: start session, send `edit …`, wait for checkpoint.
 */
export async function terminalCheckpoint(
  env: Pick<WorkbenchEnv, "origin" | "token" | "projectId">,
  text = "edit the notes",
): Promise<void> {
  const beforeRes = await api(
    env.origin,
    env.token,
    "GET",
    `/api/checkpoints?project=${encodeURIComponent(env.projectId)}`,
  );
  if (!beforeRes.ok) throw new Error(`checkpoints list failed: ${await beforeRes.text()}`);
  const before = (await beforeRes.json()) as { checkpoints: unknown[] };
  const beforeCount = before.checkpoints.length;

  const start = await api(env.origin, env.token, "POST", "/api/terminal", {
    project: env.projectId,
    provider: "fake",
  });
  if (!start.ok && start.status !== 409) {
    throw new Error(`terminal start failed: ${await start.text()}`);
  }

  const wsUrl = buildTerminalUrl(env.origin, env.token, env.projectId);
  const ws = new WebSocket(wsUrl);
  ws.binaryType = "arraybuffer";

  let pty = "";

  await new Promise<void>((resolve, reject) => {
    const deadline = Date.now() + 30_000;
    let sent = false;
    let done = false;

    const fail = (err: Error) => {
      if (done) return;
      done = true;
      clearInterval(timer);
      ws.close();
      reject(err);
    };

    const timer = setInterval(() => {
      if (Date.now() > deadline) {
        clearInterval(timer);
        fail(
          new Error(
            sent
              ? `timed out waiting for TURN-END on terminal websocket; pty=${JSON.stringify(pty)}`
              : "timed out waiting for FAKE-CLI READY on terminal websocket",
          ),
        );
      }
    }, 250);

    ws.onopen = () => {
      // wait for replay/data
    };
    ws.onmessage = (ev) => {
      if (typeof ev.data === "string") return;
      pty += Buffer.from(ev.data as ArrayBuffer).toString("utf8");
      if (!sent && pty.includes("FAKE-CLI READY")) {
        sent = true;
        ws.send(new TextEncoder().encode(`${text}\r`));
        return;
      }
      if (sent && pty.includes("TURN-END")) {
        done = true;
        clearInterval(timer);
        resolve();
      }
    };
    ws.onerror = () => fail(new Error("terminal websocket error"));
    ws.onclose = () => {
      if (!done && !pty.includes("TURN-END")) {
        fail(new Error(`terminal websocket closed early; pty=${JSON.stringify(pty)}`));
      }
    };
  });

  const pollDeadline = Date.now() + 30_000;
  while (Date.now() < pollDeadline) {
    const list = await api(
      env.origin,
      env.token,
      "GET",
      `/api/checkpoints?project=${encodeURIComponent(env.projectId)}`,
    );
    if (!list.ok) throw new Error(`checkpoints list failed: ${await list.text()}`);
    const body = (await list.json()) as { checkpoints: unknown[] };
    if (body.checkpoints.length > beforeCount) {
      ws.close();
      return;
    }
    await new Promise((r) => setTimeout(r, 100));
  }
  ws.close();
  throw new Error("timed out waiting for checkpoint after terminal turn");
}

async function signInAndClone(origin: string, token: string): Promise<string> {
  const device = await api(origin, token, "POST", "/api/github/device");
  if (!device.ok) throw new Error(`device start failed: ${await device.text()}`);

  let poll = await api(origin, token, "POST", "/api/github/device/poll");
  if (!poll.ok) throw new Error(`device poll failed: ${await poll.text()}`);
  let body = await poll.json();
  if (body.status === "pending") {
    poll = await api(origin, token, "POST", "/api/github/device/poll");
    body = await poll.json();
  }
  if (body.status !== "signedIn") {
    throw new Error(`expected signedIn, got ${JSON.stringify(body)}`);
  }

  const clone = await api(origin, token, "POST", "/api/github/clone", { repo: DEFAULT_REPO });
  if (!clone.ok) throw new Error(`clone failed: ${await clone.text()}`);
  const info = (await clone.json()) as { id: string };
  return info.id;
}

/** Boot release `workbench serve`, stub GitHub, bare remote, fake CLI, and a cloned project. */
export async function startWorkbench(): Promise<WorkbenchEnv> {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "workbench-e2e-"));
  const projectsDir = path.join(tmp, "projects");
  const gitRoot = path.join(tmp, "git");
  fs.mkdirSync(projectsDir, { recursive: true });
  const bareRemote = createBareRemote(gitRoot);
  // PRs report the branch's real tip in the bare remote, as GitHub does.
  const stub = await startGithubStub("cid", {
    headSha: (ref) => {
      try {
        return execFileSync("git", ["rev-parse", "--verify", "-q", `refs/heads/${ref}`], {
          cwd: bareRemote,
          encoding: "utf8",
        }).trim();
      } catch {
        return null;
      }
    },
  });
  const gitUrl = fileGitUrl(gitRoot);

  const root = repoRoot();
  const agents = path.join(tmp, "agents.json");
  const resolver = path.join(root, "tests/fixtures/fake_resolver.sh");
  fs.writeFileSync(
    agents,
    JSON.stringify({
      fake: [fakeCliPath(root)],
      "fake-resolver": ["sh", resolver, "needs-input"],
      "fake-garbage": ["sh", resolver, "garbage"],
    }) + "\n",
  );

  const child = spawn(
    path.join(root, "target/release/workbench"),
    [
      "serve",
      "--port",
      "0",
      "--projects-dir",
      projectsDir,
      "--token",
      E2E_TOKEN,
      "--agents-config",
      agents,
    ],
    {
      cwd: root,
      env: {
        ...process.env,
        WORKBENCH_GITHUB_CLIENT_ID: "cid",
        WORKBENCH_GITHUB_API_URL: stub.baseUrl,
        WORKBENCH_GITHUB_WEB_URL: stub.baseUrl,
        WORKBENCH_GITHUB_GIT_URL: gitUrl,
      },
      stdio: ["ignore", "pipe", "pipe"],
    },
  );

  let exited: number | null = null;
  child.on("exit", (code) => {
    exited = code;
    if (code !== null && code !== 0) {
      process.stderr.write(`workbench exited ${code}\n`);
    }
  });

  const ready = await new Promise<string>((resolve, reject) => {
    const onLine = (line: string) => {
      const m = line.match(/^workbench ready: (https?:\/\/[^\s/]+(?::\d+)?)/);
      if (m) resolve(m[1]);
    };
    const rl = readline.createInterface({ input: child.stdout! });
    rl.on("line", onLine);
    child.stderr?.on("data", (b) => process.stderr.write(b));
    child.on("error", reject);
    child.on("exit", (code) => {
      if (code !== null && code !== 0) reject(new Error(`workbench exited ${code}`));
    });
    setTimeout(() => reject(new Error("timed out waiting for workbench ready")), 120_000);
  });

  const origin = ready;
  const projectId = await signInAndClone(origin, E2E_TOKEN);
  const projectDir = path.join(projectsDir, projectId);

  return {
    origin,
    token: E2E_TOKEN,
    projectId,
    projectDir,
    bareRemote,
    tmp,
    github: {
      pullsPosts: () => stub.pullsPosts(),
      setPullsStatus: (status) => stub.setPullsStatus(status),
      setPrStatus: (patch) => stub.setPrStatus(patch),
      baseUrl: stub.baseUrl,
      setMainProtected: (isProtected) => stub.setMainProtected(isProtected),
      protectionGets: () => stub.protectionGets(),
      setMainRulesetRequiresPr: (requiresPr) => stub.setMainRulesetRequiresPr(requiresPr),
      readyForReviewCalls: () => stub.readyForReviewCalls(),
      setGraphqlStatus: (status) => stub.setGraphqlStatus(status),
    },
    stop: async () => {
      if (exited === null) {
        child.kill("SIGTERM");
        await new Promise<void>((r) => child.once("exit", () => r()));
      }
      await stub.close();
      fs.rmSync(tmp, { recursive: true, force: true });
    },
  };
}
