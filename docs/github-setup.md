# GitHub sign-in setup

## Easiest: use the GitHub CLI

1. Install the [GitHub CLI](https://cli.github.com) (`brew install gh`).
2. Run `gh auth login` once and follow the prompts.
3. Start `orso serve`. You are signed in: your avatar and name appear in the top-right,
   and **Open a project** lists your repositories.

No OAuth App or client id is needed, and orso never stores your token: it runs
`gh api …` for your profile, repositories and pull requests (Share preview, Ready for review,
PR status), and git asks `gh auth git-credential` for credentials when cloning and pushing. Commits are authored with your global git name and email
(`git config --global user.name` / `user.email`) when set, otherwise your GitHub name and
noreply address. There is no Sign out in this mode; use `gh auth logout`. (A
logged-in `gh` is re-checked about once a minute, a logged-out one every few seconds.)

The daemon runs `gh` from its own environment, so start it from a shell where `gh` is on
`PATH`. Launching it from a GUI app or launchd may not see `/opt/homebrew/bin`.

## Optional: in-app sign-in without the GitHub CLI

Done once per organisation by an admin. Only needed for people who do not use `gh`.
An in-app session takes priority over the CLI login.

1. GitHub → Settings → Developer settings → **OAuth Apps** → *New OAuth App*
   (for an organisation: Organisation settings → Developer settings).
   - Application name: `orso`
   - Homepage URL: your site or repo URL
   - Authorization callback URL: any URL you control (the device flow does not use it)
2. Open the app and tick **Enable Device Flow**. Copy the **Client ID** (no secret is needed).
3. If the organisation restricts OAuth apps, approve orso under
   Organisation settings → Third-party access.

Everyone then starts the daemon with the client id:

```sh
ORSO_GITHUB_CLIENT_ID=<client-id> orso serve
# or: orso serve --github-client-id <client-id>
```

**Sign in with GitHub** appears in the top-right. orso asks for the `repo` scope
(clone and push the site repositories) and `read:user` (your name on commits). The token is
stored in `.orso/github.json` inside your projects directory (`~/orso-projects` by
default), readable only by you. **Sign out** deletes that file; to revoke the token on GitHub
itself, remove orso under github.com → Settings → Applications → Authorized OAuth Apps.

## Known limitations

- **Previews of opened repositories:** orso serves each preview at the root of its own
  loopback address, so a site previews with plain root-relative links and no orso settings.
  A site whose live build needs a `base` path opens at `/`; type the base path in the
  preview's address bar.
- Opening a repository runs `npm install`, which executes that repository's install scripts —
  only open repositories you trust (the same trust you need to run `npm run dev` on them).
