---
title: Publish your changes
description: Merge an approved change and send it to your live site.
category: review-publish
order: 3
---

## Publish

Once the change's pull request is approved, choose **Publish** in orso. orso merges the pull request into `main`. Your deploy then builds the site from `main` and puts it live.

With the template's [Cloudflare workflow](/help/publish-to-cloudflare/), the live site usually updates within a few minutes. Choose **Open live site** to check it.

## After publishing

Start a **New change** for your next piece of work. Keeping each change small and focused makes it easier to review and quicker to publish.

## If Publish isn't available

- The pull request may still need an approval, especially if `main` is protected. See [Protect your main branch](/help/protect-your-main-branch/).
- Checks on the pull request may have failed. Open the pull request on GitHub to see why.
- `main` may have moved on since you started. Use **Get latest** first. See [Get the latest changes](/help/get-latest-and-resolve-conflicts/).
