---
title: "orso says it can't reach the daemon"
description: What to do when the page can't connect to orso.
category: troubleshooting
order: 1
---

orso's editor runs in your browser, but it talks to the `orso` program running on your computer. If the page says it can't reach it, one of these is usually the cause.

## orso isn't running

Check the terminal where you started orso. If it has stopped, or you closed the terminal, start it again:

```sh
orso serve --open
```

## The link is out of date

Each time orso starts, it creates a new link with a new `?t=` token. An old tab or bookmark won't work after a restart. Use the new link that `orso serve --open` opens.

## Still stuck

- Run `orso --version` to check orso is installed and on your `PATH`.
- Make sure nothing else is using orso's port.
- See **Report a problem**.
