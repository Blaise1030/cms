---
title: Publish your site to Cloudflare
description: Connect the template's deploy workflow to your Cloudflare account.
category: new-site
order: 3
---

The template deploys to Cloudflare Workers with GitHub Actions. Every push to `main` goes live, and every pull request gets its own preview link.

You only set this up once per site.

## 1. Get your Cloudflare details

1. Sign in to the Cloudflare dashboard.
2. Copy your **Account ID**. You can find it on the account home page.
3. Create an API token with the **Workers Scripts: Edit** permission.

## 2. Add them to your GitHub repo

In your site's repo on GitHub, go to **Settings → Secrets and variables → Actions** and add:

| Type | Name | Value |
| --- | --- | --- |
| Secret | `CLOUDFLARE_API_TOKEN` | The API token |
| Secret | `CLOUDFLARE_ACCOUNT_ID` | Your account ID |
| Variable (optional) | `CLOUDFLARE_WORKER_NAME` | The Worker's name. Defaults to `name` in `wrangler.toml` |

## 3. Deploy

Publish a change, or re-run the **Deploy Cloudflare Workers** workflow from the repo's **Actions** tab. When it finishes, **Open live site** in orso takes you to your site.

## How orso finds your links

The workflow records each deploy as a GitHub deployment with its URL. orso reads those deployments to show **Open live site** and each change's preview link. If the links are missing, check that the workflow ran successfully.

## Using another host

Any host works as long as it deploys from `main`. For orso to show preview and live links, the deploy has to record GitHub deployments with an `environment_url`.
