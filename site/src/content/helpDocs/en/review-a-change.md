---
title: Review a change
description: Look over a change, leave comments and approve it.
category: review-publish
order: 2
---

Reviews happen on the change's pull request, so reviewers can use orso or GitHub.

## As a reviewer

1. Open the preview link and click through the changed pages.
2. Leave comments on anything that needs fixing. Be specific: say which page, what's wrong and what you expected.
3. When it looks right, approve the pull request.

## Screenshots in comments

orso can attach screenshots to review comments. It stores the images on a branch called `orso-review-assets` in your repo, and deletes the branch when no open change needs it. You can also delete it yourself at any time.

For this to work, you need push access to the repo. If the site's preview builds set `ORSO_PREVIEW=1`, orso can take the screenshots without asking to share your screen. Never set it for production builds.

## As the author

Read each comment and fix what's needed, yourself or with the assistant. Share again to update the pull request, then let your reviewer know it's ready for another look.
