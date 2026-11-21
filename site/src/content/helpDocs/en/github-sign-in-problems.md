---
title: GitHub sign-in fails or repos are missing
description: Fix sign-in problems and missing repositories.
category: troubleshooting
order: 2
---

For the setup steps, see [Sign in with GitHub](/help/sign-in-with-github/).

## Using the GitHub CLI

- Run `gh auth status` in a terminal. If you aren't signed in, run `gh auth login`.
- Start orso from a terminal where `gh` works. If orso is started some other way, it may not find `gh`.
- After signing in with `gh`, wait a few seconds. orso checks for a new sign-in regularly.

## Using in-app sign-in

- Make sure orso was started with the right `ORSO_GITHUB_CLIENT_ID`.
- Make sure the OAuth app has **Enable Device Flow** ticked.
- If your organisation restricts OAuth apps, an admin needs to approve orso under the organisation's **Third-party access** settings.

## A repo is missing from the list

- You need write access to the repo. Ask its owner to add you.
- For organisation repos, the organisation may need to approve orso's access.

## Pushing or opening pull requests fails

Check that you still have write access, and that your sign-in hasn't expired. Signing in again usually fixes it.
