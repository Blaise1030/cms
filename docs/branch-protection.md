# Protect `main` so merges need review

Workbench publishes every change as a draft pull request into `main`. Without a branch
protection rule, anyone with push access can merge that pull request (or push to `main`
directly) without a review. Workbench never sets protection for you; a repository admin
does it once in GitHub.

## When Workbench shows the hint

After **Share preview**, Workbench asks GitHub about `main` in two ways at once: classic
branch protection (`GET /repos/{owner}/{repo}/branches/main/protection`) and repository
rulesets (`GET /repos/{owner}/{repo}/rules/branches/main`).

| GitHub answers | Meaning | Hint |
|----------------|---------|------|
| classic 200, or a ruleset with a "require a pull request" rule | `main` is protected | hidden |
| classic 404 "Branch not protected", and no ruleset requires a pull request | `main` is unprotected | shown |
| 403, 404 "Not Found", any error, or no answer within 3 seconds | you are not an admin, or GitHub did not say | hidden |

**Dismiss** hides the hint for that project for good. Workbench remembers it in the
project registry (`{projects-dir}/.workbench/projects.json`), so it stays hidden after a
reload or restart. Other projects still show it.

## Add the rule

1. Open the hint's **Branch settings** link, or on GitHub go to the repository →
   **Settings** → **Branches** (or **Rules** → **Rulesets**).
2. **Add branch protection rule** (or **New branch ruleset**) for `main`.
3. Tick **Require a pull request before merging** and **Require approvals** (1 or more).
4. Optional: **Do not allow bypassing the above settings**, so admins also go through review.
5. Save.

The next **Share preview** on this repository will not show the hint.
