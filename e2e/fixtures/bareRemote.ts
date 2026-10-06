import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

export const DEFAULT_REPO = "acme/site";

const PACKAGE_JSON =
  JSON.stringify(
    {
      name: "site",
      private: true,
      scripts: {
        dev:
          'sh -c \'echo "  ➜  Local:   http://127.0.0.1:$PORT$PUBLIC_BASE"; exec sleep 3600\'',
      },
    },
    null,
    2,
  ) + "\n";

function git(cwd: string, args: string[]) {
  execFileSync("git", args, { cwd, stdio: "pipe" });
}

/** `{root}/acme/site.git` — bare remote with a seeded `main` branch. */
export function createBareRemote(root: string, repo = DEFAULT_REPO): string {
  const [owner, name] = repo.split("/");
  const src = path.join(root, "src-repo");
  fs.mkdirSync(src, { recursive: true });
  fs.writeFileSync(path.join(src, "package.json"), PACKAGE_JSON);
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
