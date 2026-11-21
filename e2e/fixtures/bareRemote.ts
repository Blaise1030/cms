import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const DEFAULT_REPO = "acme/site";

const PACKAGE_JSON =
  JSON.stringify(
    {
      name: "site",
      private: true,
      scripts: {
        // Tiny static server (`serve.mjs`) so the preview frame shows the working tree.
        dev: "node serve.mjs",
      },
    },
    null,
    2,
  ) + "\n";

/**
 * Stand-in dev server: serves files from the working tree at its root (`/` → `index.html`,
 * 404 otherwise), as orso's preview origin expects, and prints the Astro-style ready line.
 * Like the `orso()` integration, it puts orso's preview bridge (`orso-bridge.js`, copied from
 * `packages/astro/src`) into every HTML page, talking to the app origins the daemon names in
 * `ORSO_APP_ORIGINS`: the preview frame is cross-origin, so the app needs it for the address bar,
 * history, picking and Page details. The supervisor appends `--port N --host 127.0.0.1 --ignore-lock`.
 */
const SERVE_MJS = `import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";

const args = process.argv.slice(2);
const i = args.indexOf("--port");
const port = Number(i >= 0 ? args[i + 1] : process.env.PORT);
const origins = (process.env.ORSO_APP_ORIGINS || "").split(",").filter(Boolean);
const bridge =
  "<script>(() => {\\nconst __orsoAppOrigins = " +
  JSON.stringify(origins) +
  ";\\n" +
  fs.readFileSync(path.join(process.cwd(), "orso-bridge.js"), "utf8") +
  "\\n})();</script>";

http
  .createServer((req, res) => {
    let p = decodeURIComponent(new URL(req.url, "http://x").pathname);
    if (p.endsWith("/")) p += "index.html";
    const file = path.join(process.cwd(), path.normalize(p));
    fs.readFile(file, (err, data) => {
      if (err) {
        res.writeHead(404, { "content-type": "text/plain" });
        res.end("Not found");
        return;
      }
      const html = file.endsWith(".html");
      res.writeHead(200, { "content-type": (html ? "text/html" : "text/plain") + "; charset=utf-8" });
      res.end(html ? bridge + data : data);
    });
  })
  .listen(port, "127.0.0.1", () => {
    console.log("  ➜  Local:   http://127.0.0.1:" + port + "/");
  });
`;

/** orso's preview bridge (with self-capture), as the `orso()` integration injects it. */
function bridgeSource(): string {
  const src = (name: string) =>
    fs.readFileSync(path.join(path.dirname(fileURLToPath(import.meta.url)), "../../packages/astro/src", name), "utf8");
  return `${src("capture.js")}\n${src("bridge.js")}\n`;
}

function git(cwd: string, args: string[]) {
  execFileSync("git", args, { cwd, stdio: "pipe" });
}

/** `{root}/acme/site.git` — bare remote with a seeded `main` branch. */
export function createBareRemote(root: string, repo = DEFAULT_REPO): string {
  const [owner, name] = repo.split("/");
  const src = path.join(root, "src-repo");
  fs.mkdirSync(src, { recursive: true });
  fs.writeFileSync(path.join(src, "package.json"), PACKAGE_JSON);
  fs.writeFileSync(path.join(src, "serve.mjs"), SERVE_MJS);
  fs.writeFileSync(path.join(src, "orso-bridge.js"), bridgeSource());
  // So `npm install` / the stubbing `dev` script leave the tree clean for change create.
  fs.writeFileSync(
    path.join(src, ".gitignore"),
    ["node_modules/", "package-lock.json", "dist/", ".astro/", ".DS_Store", ""].join("\n"),
  );
  git(src, ["init", "-b", "main"]);
  git(src, ["-c", "user.name=Grace", "-c", "user.email=g@example.com", "add", "-A"]);
  git(src, [
    "-c",
    "user.name=Grace",
    "-c",
    "user.email=g@example.com",
    "commit",
    "-m",
    "init",
  ]);
  fs.mkdirSync(path.join(root, owner), { recursive: true });
  const bare = path.join(root, owner, `${name}.git`);
  git(root, ["clone", "--bare", src, path.join(owner, `${name}.git`)]);
  return bare;
}

/** `file://` URL base pointing at `root` (no trailing slash). */
export function fileGitUrl(root: string): string {
  return `file://${root}`;
}
