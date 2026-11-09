#!/usr/bin/env node
// Dev-only generator for agent-runtime.lock.json.
//
// Queries npm, nodejs.org, GitHub Releases and Cursor's installer, downloads
// each pinned artifact, and computes real SHA-256 values and sizes. The
// generated lock is committed; CI and tests never run this script.
//
//   node scripts/update-agent-runtimes.mjs
//   node scripts/update-agent-runtimes.mjs --cache-dir /tmp/runtime-cache
//
// Requires Node 20+ and the system `tar` command.

import { createWriteStream } from "node:fs";
import { createReadStream } from "node:fs";
import { mkdir, readdir, rm, stat, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { execFile } from "node:child_process";
import { Readable } from "node:stream";
import { finished } from "node:stream/promises";
import { promisify } from "node:util";
import os from "node:os";
import path from "node:path";
import process from "node:process";

const execFileAsync = promisify(execFile);
const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const LOCK_PATH = path.join(ROOT, "agent-runtime.lock.json");
const TARGETS = ["darwin-arm64", "darwin-x64"];
const NPM_REGISTRY = "https://registry.npmjs.org";

function arg(name, fallback) {
  const index = process.argv.indexOf(name);
  return index === -1 ? fallback : process.argv[index + 1];
}

const CACHE = path.resolve(
  arg("--cache-dir", path.join(os.tmpdir(), "orso-agent-runtime-cache")),
);

async function fetchJson(url) {
  const response = await fetch(url, {
    headers: { "user-agent": "orso-agent-runtime-lock/1" },
  });
  if (!response.ok) {
    throw new Error(`${response.status} ${response.statusText} from ${url}`);
  }
  return response.json();
}

async function fetchText(url) {
  const response = await fetch(url, {
    headers: { "user-agent": "orso-agent-runtime-lock/1" },
  });
  if (!response.ok) {
    throw new Error(`${response.status} ${response.statusText} from ${url}`);
  }
  return response.text();
}

async function sha256File(file) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(file)) {
    hash.update(chunk);
  }
  return hash.digest("hex");
}

async function download(url, cacheName) {
  await mkdir(CACHE, { recursive: true });
  const file = path.join(CACHE, cacheName);
  try {
    const info = await stat(file);
    if (info.size > 0) {
      return { file, sha256: await sha256File(file), bytes: info.size };
    }
  } catch {
    // Cache miss.
  }

  const response = await fetch(url, {
    headers: { "user-agent": "orso-agent-runtime-lock/1" },
  });
  if (!response.ok || !response.body) {
    throw new Error(`${response.status} ${response.statusText} from ${url}`);
  }
  const hash = createHash("sha256");
  const output = createWriteStream(file);
  let bytes = 0;
  for await (const chunk of Readable.fromWeb(response.body)) {
    hash.update(chunk);
    bytes += chunk.length;
    if (!output.write(chunk)) {
      await new Promise((resolve) => output.once("drain", resolve));
    }
  }
  output.end();
  await finished(output);
  return { file, sha256: hash.digest("hex"), bytes };
}

async function extract(archive, directory) {
  await rm(directory, { recursive: true, force: true });
  await mkdir(directory, { recursive: true });
  await execFileAsync("tar", ["-xzf", archive, "-C", directory]);
  return directory;
}

async function walkFiles(root, current = root, out = []) {
  const entries = await readdir(current, { withFileTypes: true });
  entries.sort((left, right) => left.name.localeCompare(right.name));
  for (const entry of entries) {
    const absolute = path.join(current, entry.name);
    if (entry.isDirectory()) {
      await walkFiles(root, absolute, out);
    } else if (entry.isFile()) {
      out.push({
        absolute,
        relative: path.relative(root, absolute).split(path.sep).join("/"),
      });
    }
  }
  return out;
}

async function installedTree(directory) {
  const files = await walkFiles(directory);
  let bytes = 0;
  for (const file of files) {
    bytes += (await stat(file.absolute)).size;
  }
  return { files, bytes };
}

async function tarInstalledBytes(archive) {
  const { stdout } = await execFileAsync("tar", ["-tzvf", archive], {
    maxBuffer: 64 * 1024 * 1024,
  });
  let total = 0;
  for (const line of stdout.split("\n")) {
    if (!line.trim()) continue;
    const tokens = line.trim().split(/\s+/);
    const mode = tokens[0] ?? "";
    if (mode.startsWith("d") || mode.startsWith("l")) continue;
    const size = Number(tokens[4]);
    if (Number.isFinite(size)) total += size;
  }
  return total;
}

async function npmLatest(pkg) {
  const encoded = encodeURIComponent(pkg);
  return fetchJson(`${NPM_REGISTRY}/${encoded}/latest`);
}

async function npmArtifact(pkg, executablePath) {
  const meta = await npmLatest(pkg);
  const asset = meta.dist.tarball.split("/").pop();
  const url = meta.dist.tarball;
  const downloaded = await download(url, `${pkg.replace("/", "-")}-${asset}`);
  if (downloaded.sha256.length !== 64) {
    throw new Error(`bad sha for ${url}`);
  }
  const directory = await extract(
    downloaded.file,
    path.join(CACHE, `extract-${pkg.replace("/", "-")}-${meta.version}`),
  );
  const { bytes } = await installedTree(directory);
  const binaryPath = path.join(directory, executablePath);
  const binarySha256 = await sha256File(binaryPath);
  return {
    meta,
    artifact: {
      package: pkg,
      asset,
      url,
      assetSha256: downloaded.sha256,
      binarySha256,
      downloadBytes: downloaded.bytes,
      installedBytes: bytes,
      executable: executablePath.replace(/^package\//, ""),
      root: "package",
      stripComponents: 1,
      format: "tar.gz",
    },
  };
}

async function nodeLock() {
  const index = await fetchJson("https://nodejs.org/dist/index.json");
  const lts = index.find((release) => release.lts);
  if (!lts) throw new Error("no Node LTS release found");
  const version = lts.version.replace(/^v/, "");
  const shasums = await fetchText(
    `https://nodejs.org/dist/v${version}/SHASUMS256.txt`,
  );
  const hashes = new Map(
    shasums
      .trim()
      .split("\n")
      .map((line) => {
        const [hash, name] = line.trim().split(/\s+/);
        return [name, hash];
      }),
  );
  const artifacts = {};
  for (const target of TARGETS) {
    const asset = `node-v${version}-darwin-${target.endsWith("arm64") ? "arm64" : "x64"}.tar.gz`;
    const url = `https://nodejs.org/dist/v${version}/${asset}`;
    const downloaded = await download(url, asset);
    const expected = hashes.get(asset);
    if (!expected || expected !== downloaded.sha256) {
      throw new Error(`Node checksum mismatch for ${asset}`);
    }
    artifacts[target] = {
      asset,
      url,
      assetSha256: downloaded.sha256,
      downloadBytes: downloaded.bytes,
      installedBytes: await tarInstalledBytes(downloaded.file),
      executable: "bin/node",
      root: `node-v${version}-darwin-${target.endsWith("arm64") ? "arm64" : "x64"}`,
      stripComponents: 1,
      format: "tar.gz",
    };
  }
  return { version, artifacts };
}

async function opencodeLock() {
  const artifacts = {};
  for (const target of TARGETS) {
    const arch = target.endsWith("arm64") ? "arm64" : "x64";
    const pkg = `@opencode/cli-darwin-${arch}`;
    const { meta, artifact } = await npmArtifact(pkg, "package/bin/opencode");
    artifacts[target] = artifact;
    if (target === "darwin-arm64") {
      artifacts[target].version = meta.version;
    }
  }
  const version = artifacts["darwin-arm64"].version;
  for (const target of TARGETS) delete artifacts[target].version;
  return { version, registry: NPM_REGISTRY, artifacts };
}

async function claudeLock() {
  const artifacts = {};
  let sdkVersion;
  for (const target of TARGETS) {
    const arch = target.endsWith("arm64") ? "arm64" : "x64";
    const pkg = `@anthropic-ai/claude-agent-sdk-darwin-${arch}`;
    const { meta, artifact } = await npmArtifact(pkg, "package/claude");
    sdkVersion = meta.version;
    artifacts[target] = {
      ...artifact,
      executable: "bin/claude",
    };
  }
  const cli = await execFileAsync(
    path.join(
      CACHE,
      `extract-@anthropic-ai-claude-agent-sdk-darwin-arm64-${sdkVersion}`,
      "package",
      "claude",
    ),
    ["--version"],
  );
  const version = (cli.stdout.match(/\d+\.\d+\.\d+/) ?? [])[0];
  if (!version) throw new Error(`cannot parse claude version from ${cli.stdout}`);
  return {
    version,
    sdkVersion,
    registry: NPM_REGISTRY,
    artifacts,
  };
}

async function codexLock() {
  const release = await fetchJson(
    "https://api.github.com/repos/openai/codex/releases/latest",
  );
  const tag = release.tag_name;
  const version = tag.replace(/^rust-v/, "");
  const artifacts = {};
  for (const target of TARGETS) {
    const arch = target.endsWith("arm64") ? "aarch64" : "x86_64";
    const asset = `codex-package-${arch}-apple-darwin.tar.gz`;
    const releaseAsset = release.assets.find((item) => item.name === asset);
    if (!releaseAsset) throw new Error(`missing Codex asset ${asset}`);
    const digest = releaseAsset.digest?.replace(/^sha256:/, "");
    if (!digest) throw new Error(`Codex asset ${asset} has no digest`);
    const downloaded = await download(releaseAsset.browser_download_url, asset);
    if (downloaded.sha256 !== digest) {
      throw new Error(`Codex digest mismatch for ${asset}`);
    }
    artifacts[target] = {
      asset,
      url: releaseAsset.browser_download_url,
      assetSha256: digest,
      downloadBytes: releaseAsset.size,
      installedBytes: await tarInstalledBytes(downloaded.file),
      executable: "bin/codex",
      stripComponents: 0,
      format: "tar.gz",
    };
  }
  return {
    version,
    tag,
    repository: "https://github.com/openai/codex",
    artifacts,
  };
}

async function cursorLock() {
  const installer = await fetchText("https://cursor.com/install");
  const match = installer.match(
    /downloads\.cursor\.com\/lab\/([^/]+)\/\$\{OS\}\/\$\{ARCH\}\/agent-cli-package\.tar\.gz/,
  );
  const version = process.env.CURSOR_AGENT_VERSION ?? match?.[1];
  if (!version) {
    throw new Error("cannot find the Cursor agent version in the installer");
  }
  const artifacts = {};
  for (const target of TARGETS) {
    const arch = target.endsWith("arm64") ? "arm64" : "x64";
    const asset = "agent-cli-package.tar.gz";
    const url = `https://downloads.cursor.com/lab/${version}/darwin/${arch}/${asset}`;
    const downloaded = await download(url, `cursor-${version}-${arch}.tar.gz`);
    const directory = await extract(
      downloaded.file,
      path.join(CACHE, `extract-cursor-${version}-${arch}`),
    );
    const root = path.join(directory, "dist-package");
    const { files, bytes } = await installedTree(root);
    const wanted = new Map();
    for (const file of files) {
      if (
        file.relative === "cursor-agent" ||
        file.relative === "node" ||
        file.relative === "index.js"
      ) {
        wanted.set(file.relative, await sha256File(file.absolute));
      }
    }
    for (const name of ["cursor-agent", "node", "index.js"]) {
      if (!wanted.has(name)) throw new Error(`Cursor archive is missing ${name}`);
    }
    artifacts[target] = {
      asset,
      url,
      assetSha256: downloaded.sha256,
      downloadBytes: downloaded.bytes,
      installedBytes: bytes,
      executable: "cursor-agent",
      executables: ["cursor-agent", "node"],
      files: Object.fromEntries([...wanted.entries()].sort()),
      root: "dist-package",
      stripComponents: 1,
      format: "tar.gz",
    };
  }
  return {
    version,
    distribution: "https://downloads.cursor.com/lab",
    artifacts,
  };
}

async function acpPins() {
  const [claude, codex] = await Promise.all([
    npmLatest("@agentclientprotocol/claude-agent-acp"),
    npmLatest("@agentclientprotocol/codex-acp"),
  ]);
  return {
    claude: {
      package: "@agentclientprotocol/claude-agent-acp",
      version: claude.version,
    },
    codex: {
      package: "@agentclientprotocol/codex-acp",
      version: codex.version,
    },
  };
}

function assertHashes(lock) {
  for (const [id, runtime] of Object.entries(lock.runtimes)) {
    for (const [target, artifact] of Object.entries(runtime.artifacts)) {
      if (!/^[0-9a-f]{64}$/.test(artifact.assetSha256 ?? "")) {
        throw new Error(`${id}/${target} has no real asset hash`);
      }
      if (artifact.binarySha256 && !/^[0-9a-f]{64}$/.test(artifact.binarySha256)) {
        throw new Error(`${id}/${target} has no real binary hash`);
      }
      for (const [file, hash] of Object.entries(artifact.files ?? {})) {
        if (!/^[0-9a-f]{64}$/.test(hash)) {
          throw new Error(`${id}/${target}/${file} has no real file hash`);
        }
      }
      if (!artifact.downloadBytes) {
        throw new Error(`${id}/${target} has no download size`);
      }
    }
  }
}

async function main() {
  console.log(`cache: ${CACHE}`);
  const [node, opencode, claude, codex, cursor, acp] = await Promise.all([
    nodeLock(),
    opencodeLock(),
    claudeLock(),
    codexLock(),
    cursorLock(),
    acpPins(),
  ]);
  const lock = {
    schemaVersion: 1,
    runtimes: { node, opencode, claude, codex, cursor },
    acp,
  };
  assertHashes(lock);
  await writeFile(LOCK_PATH, JSON.stringify(lock, null, 2) + "\n");
  console.log(`wrote ${LOCK_PATH}`);
  for (const [id, runtime] of Object.entries(lock.runtimes)) {
    console.log(`${id}: ${runtime.version}`);
  }
}

main().catch((error) => {
  console.error(error.stack || String(error));
  process.exitCode = 1;
});
