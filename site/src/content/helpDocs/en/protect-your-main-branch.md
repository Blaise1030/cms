---
title: Protect your main branch
description: Require a review before anything is merged into main.
category: review-publish
order: 5
---

orso publishes every change as a pull request into `main`. Without branch protection, anyone with push access can merge a pull request, or push to `main` directly, without a review.

orso never changes this for you. A repo admin sets it up once on GitHub.

## Add the rule

1. On GitHub, open the repo's **Settings → Branches**, or **Settings → Rules → Rulesets**. You can also use the **Branch settings** link in orso's hint.
2. Choose **Add branch protection rule**, or **New branch ruleset**, for `main`.
3. Tick **Require a pull request before merging** and **Require approvals**, with at least one approval.
4. Optionally, tick **Do not allow bypassing the above settings**, so admins also need a review.
5. Save.

## orso's hint

After **Share preview** (see [Share a preview](../share-a-preview/)), orso checks whether `main` requires a pull request. If it doesn't, orso shows a hint. The hint only appears for repo admins, and disappears once the rule is in place.

Choose **Dismiss** to hide the hint for that project permanently. Other projects still show it.
