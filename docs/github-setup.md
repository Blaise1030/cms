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
WORKBENCH_GITHUB_CLIENT_ID=Iv1.xxxxxxxx workbench serve
# or: workbench serve --github-client-id Iv1.xxxxxxxx
```

**Sign in with GitHub** appears in the top-right. Workbench asks for the `repo` scope
(clone and push the site repositories) and `read:user` (your name on commits). The token is
stored in `~/workbench-projects/.workbench/github.json`, readable only by you; **Sign out**
deletes it.
