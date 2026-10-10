# Protect `main` so merges need review

Workbench publishes every change as a draft pull request into `main`. Without a branch
protection rule, anyone with push access can merge that pull request (or push to `main`
directly) without a review. Workbench never sets protection for you; a repository admin
does it once in GitHub.

## When Workbench shows the hint

After **Share preview**, Workbench asks GitHub for the protection of `main`
(`GET /repos/{owner}/{repo}/branches/main/protection`):

| GitHub answers | Meaning | Hint |
|----------------|---------|------|
| 200 | `main` is protected | hidden |
| 404 | `main` has no protection rule | shown |
| 403 or any error | you are not an admin, so GitHub will not say | hidden |

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
