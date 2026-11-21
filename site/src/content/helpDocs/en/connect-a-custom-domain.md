---
title: Connect a custom domain
description: Serve your Cloudflare-hosted site from your own domain.
category: new-site
order: 4
---

Your site first goes live on a `workers.dev` address. To use your own domain:

1. Add the domain to your Cloudflare account, if it isn't there already, and follow Cloudflare's steps to point its nameservers at Cloudflare.
2. In the Cloudflare dashboard, open **Workers & Pages** and choose your site's Worker.
3. Go to **Settings → Domains & Routes** and choose **Add → Custom domain**.
4. Enter your domain, such as `www.example.com`, and confirm.

Cloudflare creates the DNS record and certificate for you. It can take a few minutes before the domain works.

## Update the site address

Set your site's public address so links, the sitemap and social cards use the new domain. In the template, that is the `url` in `src/site.ts`. You can ask the assistant to "change the site URL to https://www.example.com".

Publish the change, and **Open live site** will use the new address after the next deploy.
