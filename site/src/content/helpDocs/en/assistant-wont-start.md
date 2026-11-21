---
title: The assistant won't start
description: Fix problems starting or running the AI assistant.
category: troubleshooting
order: 3
---

## Reinstall the provider

1. Open the assistant's setup screen.
2. Choose your provider and reinstall it.

orso downloads a fresh copy and checks it before using it.

## Check your account

Claude Code, Codex and Cursor need an account with that provider. Make sure you are signed in and your plan includes access. If you don't have one, switch to **OpenCode**, which works without an account.

## Where orso keeps the tools

orso stores its copies of the assistant tools here:

- **macOS:** `~/Library/Application Support/orso/runtimes`
- **Linux:** orso's folder in your XDG data directory

orso checks these files every time it starts and refuses to use a damaged copy. Reinstalling from the setup screen replaces it.

## Downloads keep failing

orso needs internet access to download the tools the first time. If you are behind a proxy or firewall, check it allows downloads from nodejs.org, npm and GitHub.
