---
title: Share a preview
description: Push your change and open a pull request others can review.
category: review-publish
order: 1
---

When your change is ready for others to see, choose **Share preview**.

## What happens

1. orso pushes your change's branch to GitHub.
2. It opens a draft pull request into `main`, with the change's title.
3. If your site uses the template's deploy workflow, the pull request gets its own preview link. orso shows that link once the deploy finishes.

Send the preview link to anyone who needs to see the change. They don't need orso to open it.

## Keep working

You can keep editing after you share. Each time you share again, orso pushes your latest work to the same pull request, and the preview updates.

## Mark it ready for review

A new pull request starts as a draft. When you are finished, mark it ready for review so your reviewers know to look.

## Labels

orso adds the `orso` label to its pull requests so reviewers can filter them. You can change this with `prLabels` in `orso.json`.

## A hint about protecting main

If anyone with access can merge into `main` without a review, orso shows a hint after you share. See **Protect your main branch**.
