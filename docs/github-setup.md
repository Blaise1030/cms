# GitHub sign-in setup

Done once per organisation by an admin.

1. GitHub → Settings → Developer settings → **OAuth Apps** → *New OAuth App*
   (for an organisation: Organisation settings → Developer settings).
   - Application name: `Workbench`
   - Homepage URL: your site or repo URL
   - Authorization callback URL: any URL you control (the device flow does not use it)
2. Open the app and tick **Enable Device Flow**. Copy the **Client ID** (no secret is needed).
3. If the organisation restricts OAuth apps, approve Workbench under
   Organisation settings → Third-party access.

Everyone then starts the daemon with the client id:

```sh
WORKBENCH_GITHUB_CLIENT_ID=<client-id> workbench serve
# or: workbench serve --github-client-id <client-id>
```

**Sign in with GitHub** appears in the top-right. Workbench asks for the `repo` scope
(clone and push the site repositories) and `read:user` (your name on commits). The token is
stored in `.workbench/github.json` inside your projects directory (`~/workbench-projects` by
default), readable only by you. **Sign out** deletes that file; to revoke the token on GitHub
itself, remove Workbench under github.com → Settings → Applications → Authorized OAuth Apps.

## Known limitations

- **Previews of opened repositories:** the live preview expects the site to accept the
  workbench's preview base path (`/preview/<id>/`), which the Workbench Astro template does.
  Other repositories open and can be edited, but their preview may not load until their
  Astro config reads `PUBLIC_BASE` the same way.
- Opening a repository runs `npm install`, which executes that repository's install scripts —
  only open repositories you trust (the same trust you need to run `npm run dev` on them).
