import { execFileSync, spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import readline from "node:readline";
import { fileURLToPath } from "node:url";

import { createBareRemote, DEFAULT_REPO, fileGitUrl } from "./bareRemote";
import { startGithubStub } from "./githubStub";

export const E2E_TOKEN = "a".repeat(64);
/** The stream-json turn the e2e `cursor` agent replays (after appending to `notes.md`). */
export const CURSOR_TURN_FIXTURE = "tests/fixtures/chat/cursor/turn.jsonl";

export type orsoEnv = {
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

function fakeAcpPath(root = repoRoot()): string {
  return path.join(root, "e2e/fixtures/fake_acp.py");
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
  env: Pick<orsoEnv, "origin" | "token" | "projectId">,
  title: string,
): Promise<void> {
  const res = await api(env.origin, env.token, "POST", "/api/changes", { project: env.projectId, title });
  if (!res.ok) throw new Error(`create change failed: ${res.status} ${await res.text()}`);
}

/**
 * Start a change the way the app's Content and Design "Create change" prompts do. The Changes
 * menu and command palette hand "New change" to the coding agent instead, which the fake ACP
 * agent cannot act on, so specs start changes here and reload the page to pick them up.
 */
export async function startChange(
  env: Pick<orsoEnv, "origin" | "token" | "projectId">,
  title: string,
): Promise<{ branch: string; title: string }> {
  const res = await api(env.origin, env.token, "POST", "/api/changes", {
    project: env.projectId,
    title,
  });
  if (!res.ok) throw new Error(`start change failed: ${await res.text()}`);
  return (await res.json()) as { branch: string; title: string };
}

export type ChangesUpdate = {
  mainUpdated: boolean;
  updated: { branch: string; title: string }[];
  conflicts: { branch: string; title: string }[];
  skipped?: { branch: string } | null;
};

/**
 * The daemon's Get latest. The app's Sync button hands this to the coding agent instead, so specs
 * call it here and reload the page, which opens any conflict it left.
 */
export async function getLatest(
  env: Pick<orsoEnv, "origin" | "token" | "projectId">,
): Promise<ChangesUpdate> {
  const res = await api(env.origin, env.token, "POST", "/api/changes/update", {
    project: env.projectId,
  });
  if (!res.ok) throw new Error(`get latest failed: ${await res.text()}`);
  return (await res.json()) as ChangesUpdate;
}

/**
 * Drive a fake ACP chat turn: create a chat with the `fake` agent, send `edit …`, wait for the
 * checkpoint the daemon saves when the turn ends.
 */
export async function chatCheckpoint(
  env: Pick<orsoEnv, "origin" | "token" | "projectId">,
  text = "edit the notes",
): Promise<void> {
  const checkpoints = async () => {
    const res = await api(
      env.origin,
      env.token,
      "GET",
      `/api/checkpoints?project=${encodeURIComponent(env.projectId)}`,
    );
    if (!res.ok) throw new Error(`checkpoints list failed: ${await res.text()}`);
    return (await res.json()) as { checkpoints: unknown[] };
  };
  const before = await checkpoints();

  const project = encodeURIComponent(env.projectId);
  const created = await api(env.origin, env.token, "POST", `/api/chat/${project}`, {
    provider: "fake",
  });
  if (!created.ok) throw new Error(`chat create failed: ${await created.text()}`);
  const { id } = (await created.json()) as { id: string };

  const sent = await api(env.origin, env.token, "POST", `/api/chat/${project}/${id}/message`, { text });
  if (!sent.ok) throw new Error(`chat message failed: ${await sent.text()}`);

  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    const body = await checkpoints();
    if (body.checkpoints.length > before.checkpoints.length) return;
    await new Promise((r) => setTimeout(r, 100));
  }
  throw new Error("timed out waiting for a checkpoint after the chat turn");
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

/** Boot release `orso serve`, stub GitHub, bare remote, fake CLI, and a cloned project. */
export async function startorso(): Promise<orsoEnv> {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "orso-e2e-"));
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
  const acp = fakeAcpPath(root);
  fs.writeFileSync(
    agents,
    JSON.stringify({
      fake: {
        command: ["python3", acp, "edit"],
        acp: ["python3", acp, "edit"],
      },
      "fake-resolver": ["sh", resolver, "needs-input"],
      "fake-garbage": ["sh", resolver, "garbage"],
      cursor: {
        command: ["python3", acp, "chat", path.join(root, CURSOR_TURN_FIXTURE)],
        acp: ["python3", acp, "chat", path.join(root, CURSOR_TURN_FIXTURE)],
      },
    }) + "\n",
  );

  const child = spawn(
    path.join(root, "target/release/orso"),
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
        ORSO_GITHUB_CLIENT_ID: "cid",
        ORSO_GITHUB_API_URL: stub.baseUrl,
        ORSO_GITHUB_WEB_URL: stub.baseUrl,
        ORSO_GITHUB_GIT_URL: gitUrl,
      },
      stdio: ["ignore", "pipe", "pipe"],
    },
  );

  let exited: number | null = null;
  child.on("exit", (code) => {
    exited = code;
    if (code !== null && code !== 0) {
      process.stderr.write(`orso exited ${code}\n`);
    }
  });

  const ready = await new Promise<string>((resolve, reject) => {
    const onLine = (line: string) => {
      const m = line.match(/^orso ready: (https?:\/\/[^\s/]+(?::\d+)?)/);
      if (m) resolve(m[1]);
    };
    const rl = readline.createInterface({ input: child.stdout! });
    rl.on("line", onLine);
    child.stderr?.on("data", (b) => process.stderr.write(b));
    child.on("error", reject);
    child.on("exit", (code) => {
      if (code !== null && code !== 0) reject(new Error(`orso exited ${code}`));
    });
    setTimeout(() => reject(new Error("timed out waiting for orso ready")), 120_000);
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
