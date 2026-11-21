---
title: Sign in with GitHub
description: Connect orso to GitHub so it can open your repos and create pull requests.
category: getting-started
order: 3
---

orso uses GitHub to clone your site, push your changes and open pull requests. There are two ways to sign in. If something goes wrong, see [GitHub sign-in fails or repos are missing](../github-sign-in-problems/).

## Easiest: the GitHub CLI

1. Install the GitHub CLI: `brew install gh`, or download it from cli.github.com.
2. Run `gh auth login` once and follow the prompts.
3. Start orso with `orso serve --open`.

Your avatar and name appear in the top-right, and **Open a project** lists your repos.

orso never stores your token in this mode. It asks `gh` whenever it needs GitHub. To sign out, run `gh auth logout`.

Start orso from a terminal where `gh` works. If you launch it some other way, it may not find `gh`.

## Without the GitHub CLI

An organisation admin can set up in-app sign-in once:

1. On GitHub, go to **Settings → Developer settings → OAuth Apps → New OAuth App**. For an organisation, use the organisation's settings.
2. Name it `orso`. Any homepage URL and callback URL you control will do.
3. Tick **Enable Device Flow** and copy the **Client ID**. You don't need a secret.
4. If your organisation restricts OAuth apps, approve orso under **Third-party access**.

Everyone then starts orso with that client ID:

```sh
ORSO_GITHUB_CLIENT_ID=<client-id> orso serve --open
```

Choose **Sign in with GitHub** in the top-right and enter the code GitHub shows you. orso asks for the `repo` scope (to clone and push) and `read:user` (to put your name on commits).

## Who your commits are from

Commits use your global Git name and email when they are set (`git config --global user.name` and `user.email`). Otherwise they use your GitHub name and its noreply address.

## You need write access

To edit a site that already lives on GitHub, you need write access to its repo.
