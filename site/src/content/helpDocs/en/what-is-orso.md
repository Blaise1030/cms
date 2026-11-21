---
title: What is orso?
description: A site builder that runs on your computer and keeps your site in your own Git repo.
category: getting-started
order: 1
---

orso is a site builder you run on your own computer. You edit your site's pages in a visual editor, ask a coding agent to make changes, see every change in a live preview, and publish through an ordinary GitHub pull request.

## How it works

- **Your site is a plain Astro project.** It lives in a Git repo on GitHub that you own. If you stop using orso, the site still builds and deploys without it.
- **orso runs locally.** It is a single program, `orso`, that serves its editor in your browser. It only listens on your own computer.
- **Every edit happens on a change.** A change is a Git branch with a readable title. Your live site never changes until you publish.
- **Publishing is a pull request.** You [share a preview](/help/share-a-preview/), someone [reviews it](/help/review-a-change/), and [publishing](/help/publish-your-changes/) merges it into `main`, which deploys your live site.

## What you need

- A Mac or a Linux computer. Windows is not supported yet.
- A GitHub account.

orso downloads its own copy of Node and the assistant tools the first time it runs, so you don't need to install anything else.

## Where to go next

- [Install orso](/help/install-orso/) to get set up.
- [Create your first site](/help/create-your-first-site/) if you are starting from scratch.
- [Is my site compatible?](/help/is-my-site-compatible/) if you already have an Astro site.

orso is free and open source under the MIT licence.
