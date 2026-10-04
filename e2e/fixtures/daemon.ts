import { spawn } from "node:child_process";
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
  stop: () => Promise<void>;
};

function repoRoot(): string {
  return path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
}

async function api(
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

/** Boot `cargo run -- serve`, stub GitHub, bare remote, and a cloned project. */
export async function startWorkbench(): Promise<WorkbenchEnv> {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "workbench-e2e-"));
  const projectsDir = path.join(tmp, "projects");
  const gitRoot = path.join(tmp, "git");
  fs.mkdirSync(projectsDir, { recursive: true });
  const bareRemote = createBareRemote(gitRoot);
  const stub = await startGithubStub();
  const gitUrl = fileGitUrl(gitRoot);

  const root = repoRoot();
  const agents = path.join(tmp, "agents.json");
  fs.writeFileSync(agents, "{}\n");

  const child = spawn(
    "cargo",
    [
      "run",
      "--release",
      "--quiet",
      "--",
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
    stop: async () => {
      child.kill("SIGTERM");
      await new Promise<void>((r) => child.once("exit", () => r()));
      await stub.close();
      fs.rmSync(tmp, { recursive: true, force: true });
    },
  };
}
