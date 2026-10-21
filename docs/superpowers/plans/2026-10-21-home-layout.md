# Home Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the main page's hero with three sections — Open a project (inline GitHub repository browser), Create from a template (card grid), and Recent — under a corner mark and account avatar.

**Architecture:** The repository search/list/open logic moves out of the `OpenRepoDialog` modal into a `RepoBrowser` component rendered on the page. A `TemplateGallery` component owns the template card and asks the page to open the existing `NewProjectDialog`. `ProjectsGrid` becomes a thin composition with one header row plus the three sections. The daemon starts returning the push timestamp it already fetches from GitHub.

**Tech Stack:** Vue 3 (`<script setup>`), Pinia, vue-router, Tailwind v4, Vitest + happy-dom, Rust/axum daemon.

**Spec:** `docs/superpowers/specs/2026-10-21-home-layout-design.md`

## Global Constraints

- No new npm or cargo dependencies.
- Every task ends with a green test run and a commit; commits are `feat:`/`fix:`/`test:` per repo style.
- Web test command: `pnpm -C web exec vitest run <path>` (targeted) and `pnpm -C web test` (full).
- Type-check + PWA check: `pnpm -C web build`.
- Rust: `cargo test --lib`.
- Exact copy: headings `Open a project`, `Create from a template`, `Recent`; input placeholder `Search repositories by name`; input aria-label `Search repositories`; signed-out hint must contain `gh auth login`.
- `Recent` keeps today's behaviour exactly: status dot, name, repo subtitle, `localhost:<port>` badge, delete menu, skeleton, empty message, error + Retry.
- The macOS title bar row stays a drag region, and the avatar keeps its 4 px top offset so it lines up with the traffic lights.

## Review Focus

- A repository whose `pushedAt` is missing or null still renders a row, with no timestamp. (Task 2/3)
- A repository that is already cloned opens the existing project instead of cloning a second copy. (Task 3)
- Clicking Open twice while a clone is in flight cannot start a second clone. (Task 3)
- Signed out, no repository request is made and the sign-in prompt shows. (Task 3)
- A failed repository load shows an alert plus Retry, and Retry refetches. (Task 3)
- A future timestamp (clock skew) renders "in 2 hours" rather than crashing. (Task 2)

---

### Task 1: Expose the push timestamp from the daemon

**Files:**
- Modify: `src/github.rs` (`RepoInfo` ~line 101, `impl From<ApiRepo>` ~145, `struct ApiRepo` ~165, `mod tests` ~1689)
- Test: `src/github.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `RepoInfo { ..., pushed_at: Option<String> }`, serialized as `pushedAt` (camelCase already applies).

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `src/github.rs`:

```rust
#[test]
fn repo_info_exposes_the_push_time() {
    let api: ApiRepo = serde_json::from_str(
        r#"{"full_name":"acme/site","private":false,"default_branch":"main","pushed_at":"2026-10-20T12:00:00Z"}"#,
    )
    .unwrap();
    let info = RepoInfo::from(api);
    assert_eq!(info.pushed_at.as_deref(), Some("2026-10-20T12:00:00Z"));
    let json = serde_json::to_value(&info).unwrap();
    assert_eq!(json["pushedAt"], "2026-10-20T12:00:00Z");
}

#[test]
fn repo_info_tolerates_a_missing_push_time() {
    let api: ApiRepo = serde_json::from_str(
        r#"{"full_name":"acme/site","private":false,"default_branch":"main"}"#,
    )
    .unwrap();
    assert_eq!(RepoInfo::from(api).pushed_at, None);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib repo_info`
Expected: FAIL — no field `pushed_at` on `RepoInfo` / `ApiRepo`.

- [ ] **Step 3: Implement the field**

In `struct ApiRepo`, add:

```rust
#[serde(default)]
pushed_at: Option<String>,
```

In `struct RepoInfo`, add:

```rust
/// ISO 8601 timestamp of the last push, when GitHub reports one.
pub pushed_at: Option<String>,
```

In `impl From<ApiRepo> for RepoInfo`, map `pushed_at: r.pushed_at`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib repo_info`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/github.rs
git commit -m "feat(daemon): expose each repository's last push time"
```

---

### Task 2: Add `pushedAt` to the web type and a relative-time helper

**Files:**
- Modify: `web/src/core/types.ts:384-389` (`GithubRepo`)
- Create: `web/src/lib/relativeTime.ts`
- Test: `web/src/lib/relativeTime.test.ts`

**Interfaces:**
- Consumes: `RepoInfo.pushedAt` from Task 1 (arrives as JSON `pushedAt`).
- Produces: `GithubRepo.pushedAt: string | null`; `relativeTime(iso: string | null | undefined, now?: number): string | null`.

- [ ] **Step 1: Write the failing test**

Create `web/src/lib/relativeTime.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { relativeTime } from "./relativeTime";

const now = Date.parse("2026-10-21T12:00:00Z");

describe("relativeTime", () => {
  it("uses the largest whole unit", () => {
    expect(relativeTime("2026-10-21T11:59:00Z", now)).toBe("1 minute ago");
    expect(relativeTime("2026-10-21T09:00:00Z", now)).toBe("3 hours ago");
    expect(relativeTime("2026-10-20T12:00:00Z", now)).toBe("1 day ago");
    expect(relativeTime("2026-10-14T12:00:00Z", now)).toBe("1 week ago");
    expect(relativeTime("2026-09-21T12:00:00Z", now)).toBe("1 month ago");
    expect(relativeTime("2025-10-21T12:00:00Z", now)).toBe("1 year ago");
  });

  it("says 'just now' under a minute", () => {
    expect(relativeTime("2026-10-21T11:59:30Z", now)).toBe("just now");
  });

  it("keeps a positive direction for clock skew", () => {
    expect(relativeTime("2026-10-21T14:00:00Z", now)).toBe("in 2 hours");
  });

  it("returns null when there is no usable timestamp", () => {
    expect(relativeTime(null, now)).toBeNull();
    expect(relativeTime(undefined, now)).toBeNull();
    expect(relativeTime("", now)).toBeNull();
    expect(relativeTime("not a date", now)).toBeNull();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm -C web exec vitest run src/lib/relativeTime.test.ts`
Expected: FAIL — cannot resolve `./relativeTime`.

- [ ] **Step 3: Implement the helper and widen the type**

`web/src/lib/relativeTime.ts`:

```ts
const UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ["year", 365 * 24 * 3600],
  ["month", 30 * 24 * 3600],
  ["week", 7 * 24 * 3600],
  ["day", 24 * 3600],
  ["hour", 3600],
  ["minute", 60],
];

const format = new Intl.RelativeTimeFormat("en", { numeric: "always" });

/** "3 days ago" for an ISO timestamp; null when it is missing or unparseable. */
export function relativeTime(iso: string | null | undefined, now = Date.now()): string | null {
  if (!iso) return null;
  const then = Date.parse(iso);
  if (Number.isNaN(then)) return null;
  const seconds = Math.trunc((then - now) / 1000);
  if (Math.abs(seconds) < 60) return "just now";
  const [unit, size] = UNITS.find(([, size]) => Math.abs(seconds) >= size) ?? UNITS.at(-1)!;
  return format.format(Math.trunc(seconds / size), unit);
}
```

Add to `GithubRepo` in `web/src/core/types.ts`:

```ts
pushedAt: string | null;
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `pnpm -C web exec vitest run src/lib/relativeTime.test.ts`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add web/src/lib/relativeTime.ts web/src/lib/relativeTime.test.ts web/src/core/types.ts
git commit -m "feat(web): format repository push times relatively"
```

---

### Task 3: RepoBrowser component

**Files:**
- Create: `web/src/modules/projects/RepoBrowser.vue`
- Create: `web/src/modules/projects/RepoBrowser.test.ts`
- Delete: `web/src/modules/projects/OpenRepoDialog.vue`, `web/src/modules/projects/OpenRepoDialog.test.ts`

**Interfaces:**
- Consumes: `useGithubStore()` (`user`, `deviceFlow`, `repos`, `reposLoading`, `load()`, `loadRepos()`, `signIn()`), `useProjectsStore()` (`projects`, `cloneRepo(fullName)`), `projectPath(id)` from `@/router`, `relativeTime(iso)` from Task 2.
- Produces: `<RepoBrowser />` — no props, no emits, renders its own `<h2>`. Test ids: `repo-browser`, `repo-item`, `repo-sign-in`, `repo-loading`.

- [ ] **Step 1: Write the failing tests**

Create `web/src/modules/projects/RepoBrowser.test.ts`:

```ts
// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createMemoryHistory, createRouter, type Router } from "vue-router";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import { useGithubStore } from "@/modules/github/store";
import type { Project } from "@/core/types";
import { useProjectsStore } from "./store";
import RepoBrowser from "./RepoBrowser.vue";

const repos = [
  { fullName: "acme/site", private: true, defaultBranch: "main", canPush: true, pushedAt: "2026-10-20T12:00:00Z" },
  { fullName: "acme/docs", private: false, defaultBranch: "main", canPush: false, pushedAt: null },
];

const project: Project = { id: "site", name: "site", port: 4000, devserver: "starting", autoApprove: false, repo: "acme/site" };

enableAutoUnmount(afterEach);

let router: Router;
function mountBrowser({ api = {}, signedIn = true }: { api?: Record<string, unknown>; signedIn?: boolean } = {}) {
  h.daemon = createFakeDaemon({ githubRepos: vi.fn(async () => repos), ...api });
  setActivePinia(createPinia());
  if (signedIn) useGithubStore().user = { login: "ada", id: 1, name: "Ada", avatarUrl: "" };
  router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/", component: { template: "<div />" } },
      { path: "/p/:id/:tab?", component: { template: "<div />" } },
    ],
  });
  return mount(RepoBrowser, { global: { plugins: [router] }, attachTo: document.body });
}

const items = () => [...document.body.querySelectorAll<HTMLElement>("[data-testid='repo-item']")];
const firstOpen = () => document.body.querySelector<HTMLButtonElement>("[data-testid='repo-item'] button")!;

describe("RepoBrowser", () => {
  it("lists, filters and clones a repository", async () => {
    const cloneProject = vi.fn(async () => project);
    mountBrowser({ api: { cloneProject } });
    await flushPromises();
    expect(items().map((i) => i.textContent)).toEqual([
      expect.stringContaining("acme/site"),
      expect.stringContaining("acme/docs"),
    ]);

    const filter = document.body.querySelector<HTMLInputElement>("[aria-label='Search repositories']")!;
    filter.value = "site";
    filter.dispatchEvent(new Event("input"));
    await flushPromises();
    expect(items()).toHaveLength(1);

    firstOpen().click();
    await flushPromises();
    expect(cloneProject).toHaveBeenCalledWith("acme/site");
    expect(router.currentRoute.value.path).toBe("/p/site/preview");
  });

  it("opens an already-cloned repository without cloning it again", async () => {
    const cloneProject = vi.fn();
    mountBrowser({ api: { cloneProject } });
    await flushPromises();
    useProjectsStore().projects = [{ ...project, id: "local", port: 4010 }];

    firstOpen().click();
    await flushPromises();
    expect(cloneProject).not.toHaveBeenCalled();
    expect(router.currentRoute.value.path).toBe("/p/local/preview");
  });

  it("shows the clone error and stays put", async () => {
    const cloneProject = vi.fn(async () => {
      throw new Error("clone boom");
    });
    mountBrowser({ api: { cloneProject } });
    await flushPromises();

    firstOpen().click();
    await flushPromises();
    expect(document.body.querySelector("[role='alert']")?.textContent).toContain("clone boom");
    expect(router.currentRoute.value.path).toBe("/");
  });

  it("cannot start a second clone while one is in flight", async () => {
    let release!: (p: unknown) => void;
    const cloneProject = vi.fn(() => new Promise((r) => (release = r)));
    mountBrowser({ api: { cloneProject } });
    await flushPromises();

    firstOpen().click();
    await flushPromises();
    firstOpen().click();
    await flushPromises();
    expect(cloneProject).toHaveBeenCalledTimes(1);

    release(project);
    await flushPromises();
  });

  it("shows a load error and retries", async () => {
    const githubRepos = vi
      .fn()
      .mockRejectedValueOnce(new Error("rate limited"))
      .mockResolvedValueOnce(repos);
    mountBrowser({ api: { githubRepos } });
    await flushPromises();
    expect(document.body.querySelector("[role='alert']")?.textContent).toContain("rate limited");

    [...document.body.querySelectorAll("button")].find((b) => b.textContent?.includes("Retry"))!.click();
    await flushPromises();
    expect(githubRepos).toHaveBeenCalledTimes(2);
    expect(items()).toHaveLength(2);
  });

  it("offers sign-in instead of repositories when signed out", async () => {
    mountBrowser({ signedIn: false });
    const gh = useGithubStore();
    gh.deviceFlow = true;
    await flushPromises();
    expect(document.body.querySelector("[data-testid='repo-sign-in']")).not.toBeNull();

    gh.deviceFlow = false;
    await flushPromises();
    expect(document.body.querySelector("[data-testid='repo-sign-in']")).toBeNull();
    expect(document.body.textContent).toContain("gh auth login");
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C web exec vitest run src/modules/projects/RepoBrowser.test.ts`
Expected: FAIL — cannot resolve `./RepoBrowser.vue`.

- [ ] **Step 3: Implement `RepoBrowser.vue`**

Script setup, in this order:

```ts
const gh = useGithubStore();
const projects = useProjectsStore();
const router = useRouter();

const filter = ref("");
const opening = ref<string | null>(null);
const loaded = ref(false);
const error = ref<string | null>(null);

const shown = computed(() => {
  const q = filter.value.trim().toLowerCase();
  return gh.repos.filter((r) => !q || r.fullName.toLowerCase().includes(q));
});

async function load() {
  error.value = null;
  try {
    await gh.loadRepos();
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loaded.value = true;
  }
}

async function open(fullName: string) {
  if (opening.value) return;
  const local = projects.projects.find((p) => p.repo === fullName);
  if (local) {
    await router.push(projectPath(local.id));
    return;
  }
  opening.value = fullName;
  error.value = null;
  try {
    const created = await projects.cloneRepo(fullName);
    await router.push(projectPath(created.id));
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    opening.value = null;
  }
}

onMounted(() => {
  if (gh.user) void load();
  else gh.load().catch(() => {});
});
watch(() => gh.user, (u) => { if (u) void load(); });
```

Template, inside `<section class="flex flex-col gap-3" data-testid="repo-browser">`:

- `<h2 class="text-lg font-medium">Open a project</h2>`
- Signed out (`v-if="!gh.user"`): a `<div class="flex flex-col items-start gap-2">` containing either the device-flow button `data-testid="repo-sign-in"` with label `Sign in with GitHub` (only when `gh.deviceFlow`) or the text `Run <code class="bg-muted rounded px-1">gh auth login</code> in a terminal to see your repositories.`
- Signed in:
  - `<Input v-model="filter" placeholder="Search repositories by name" aria-label="Search repositories" />`
  - `<div class="divide-y rounded-lg border" data-testid="repo-list">`
  - Spinner row `data-testid="repo-loading"` while `!loaded || gh.reposLoading`: `<Spinner class="size-4" />` + `Loading repositories…`.
  - One `div` per `shown` row, `data-testid="repo-item"`, `class="flex items-center gap-3 px-4 py-2.5"`, containing:
    - `<span class="min-w-0 flex-1 truncate text-sm">{{ r.fullName }}</span>`
    - `<span v-if="relativeTime(r.pushedAt)" class="text-muted-foreground text-xs">{{ relativeTime(r.pushedAt) }}</span>`
    - `<Button variant="outline" size="sm" :disabled="opening !== null" @click="open(r.fullName)">` with a `Spinner class="size-3.5"` while `opening === r.fullName`, else the text `Open`.
  - Empty message when not loading and `shown.length === 0`: `{{ filter.trim() ? "No repositories match." : "No repositories yet." }}`.
  - Error paragraph after the list: `<p v-if="error" class="text-destructive text-sm" role="alert">{{ error }} <Button variant="link" size="sm" @click="load()">Retry</Button></p>`

Imports needed: `computed, onMounted, ref, watch` from vue; `useRouter`; `Button`, `Input`, `Spinner`; `errorMessage`; `projectPath`; `relativeTime`; both stores.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C web exec vitest run src/modules/projects/RepoBrowser.test.ts`
Expected: PASS (6 tests).

- [ ] **Step 5: Run the full web suite**

Run: `pnpm -C web test`
Expected: PASS. `OpenRepoDialog` stays in place (still used by `ProjectsGrid`) and is deleted in Task 5, once the page no longer imports it.

- [ ] **Step 6: Commit**

```bash
git add web/src/modules/projects/RepoBrowser.vue web/src/modules/projects/RepoBrowser.test.ts
git commit -m "feat(web): browse and open repositories on the main page"
```

---

### Task 4: TemplateGallery component

**Files:**
- Create: `web/src/modules/projects/TemplateGallery.vue`
- Test: `web/src/modules/projects/TemplateGallery.test.ts`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `<TemplateGallery @create="..." />`; emits `create` with no payload. Test id `template-astro-marketing`.

- [ ] **Step 1: Write the failing test**

```ts
// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import TemplateGallery from "./TemplateGallery.vue";

describe("TemplateGallery", () => {
  it("offers the Astro template and asks the page to create", async () => {
    const w = mount(TemplateGallery);
    expect(w.text()).toContain("Create from a template");
    const card = w.get("[data-testid='template-astro-marketing']");
    expect(card.text()).toContain("Astro marketing");
    await card.trigger("click");
    expect(w.emitted("create")).toHaveLength(1);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm -C web exec vitest run src/modules/projects/TemplateGallery.test.ts`
Expected: FAIL — cannot resolve `./TemplateGallery.vue`.

- [ ] **Step 3: Implement `TemplateGallery.vue`**

`defineEmits<{ create: [] }>()`. Template:

- `<section class="flex flex-col gap-3" data-testid="template-gallery">`
- `<h2 class="text-lg font-medium">Create from a template</h2>`
- `<div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">`
- One `<button type="button" data-testid="template-astro-marketing" @click="emit('create')">` with `class="bg-card hover:bg-accent/50 flex flex-col overflow-hidden rounded-lg border text-left transition-colors"`:
  - A muted wireframe preview `<span class="bg-muted flex aspect-[16/10] flex-col gap-1.5 p-3" aria-hidden="true">` holding four placeholder bars (`bg-foreground/15`, `bg-foreground/10`) as in the spec — no screenshots.
  - A footer `<span class="flex items-center gap-2 border-t px-3 py-2.5">` with `<span class="text-sm font-medium">Astro marketing</span>`.

- [ ] **Step 4: Run the test to verify it passes**

Run: `pnpm -C web exec vitest run src/modules/projects/TemplateGallery.test.ts`
Expected: PASS (1 test).

- [ ] **Step 5: Commit**

```bash
git add web/src/modules/projects/TemplateGallery.vue web/src/modules/projects/TemplateGallery.test.ts
git commit -m "feat(web): add the template gallery card"
```

---

### Task 5: Recompose ProjectsGrid

**Files:**
- Modify: `web/src/modules/projects/ProjectsGrid.vue` (template 87-203, imports 1-45)
- Test: `web/src/modules/projects/ProjectsGrid.test.ts`

**Interfaces:**
- Consumes: `<RepoBrowser />` (Task 3), `<TemplateGallery @create />` (Task 4), existing `GithubAccount`, `PawMark`, `NewProjectDialog`, projects store.
- Produces: the page.

- [ ] **Step 1: Update the failing tests**

In `ProjectsGrid.test.ts`:

1. Replace the welcome-header test with:

```ts
it("shows the brand mark and account control in the page header", async () => {
  const w = mountGrid();
  await flushPromises();
  const header = w.get("[data-testid='page-header']");
  expect(header.find("svg.paw-mark").classes()).toContain("size-7");
  expect(header.find("[data-testid='account-slot']").exists()).toBe(true);
  expect(w.find("img[src*='orso-logo']").exists()).toBe(false);
  w.unmount();
});
```

2. Replace the "keeps the account control on the traffic-light row on desktop" test with:

```ts
it("keeps the header draggable only on desktop", async () => {
  const browser = mountGrid();
  await flushPromises();
  expect(browser.get("[data-testid='page-header']").attributes("data-tauri-drag-region")).toBeUndefined();
  browser.unmount();

  document.documentElement.dataset.desktop = "macos";
  try {
    const desktop = mountGrid();
    await flushPromises();
    const header = desktop.get("[data-testid='page-header']");
    expect(header.attributes("data-tauri-drag-region")).toBe("");
    expect(header.element.nextElementSibling).toBe(desktop.get("[data-testid='projects-scroll']").element);
    desktop.unmount();
  } finally {
    delete document.documentElement.dataset.desktop;
  }
});
```

3. Delete the old "drags the macOS desktop window from a strip along the top, and only there" test; the test above replaces it.

4. Replace the "creates a project from the dialog" test's opener: instead of `byText("New project")!.click()`, use:

```ts
await w.get("[data-testid='template-astro-marketing']").trigger("click");
await flushPromises();
```

5. Add:

```ts
it("renders the three home sections", async () => {
  const w = mountGrid();
  await flushPromises();
  for (const heading of ["Open a project", "Create from a template", "Recent"]) {
    expect(w.findAll("h2").map((h) => h.text())).toContain(heading);
  }
  w.unmount();
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C web exec vitest run src/modules/projects/ProjectsGrid.test.ts`
Expected: FAIL — no `page-header`, no section headings, no template card.

- [ ] **Step 3: Recompose the component**

Imports: drop `Plus`, `FolderGit2`, and `OpenRepoDialog`; add `RepoBrowser` and `TemplateGallery`. Remove the `showOpen` ref and the `OpenRepoDialog` tag.

Template root becomes:

```vue
<div class="flex h-full flex-col">
  <div
    class="flex h-11 shrink-0 items-start justify-between pt-1"
    :class="macDesktop ? 'pl-20 pr-3 fullscreen:hidden' : 'px-3'"
    :data-tauri-drag-region="macDesktop ? '' : undefined"
    data-testid="page-header"
  >
    <PawMark class="size-7" />
    <div data-testid="account-slot"><GithubAccount /></div>
  </div>
  <div class="relative min-h-0 flex-1 overflow-y-auto" data-testid="projects-scroll">
    <div class="mx-auto flex w-full max-w-3xl flex-col gap-12 px-10 pt-16 pb-16">
      <RepoBrowser />
      <TemplateGallery @create="showNew = true" />
      <section class="flex flex-col gap-3">
        <h2 class="text-lg font-medium">Recent</h2>
        <!-- existing error / skeleton / empty / ItemGroup markup moves here unchanged -->
      </section>
    </div>
  </div>
</div>
<NewProjectDialog v-model:open="showNew" @created="open" />
<!-- AlertDialog markup unchanged -->
```

Remove the hero (`header`, `PawMark size-12`, `<h1>orso</h1>`, tagline) and the two action buttons. Keep the Recent list markup, its `data-testid="project-grid"`, and all existing states verbatim.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C web exec vitest run src/modules/projects/ProjectsGrid.test.ts`
Expected: PASS.

- [ ] **Step 5: Delete the now-unused dialog and run the full suite**

```bash
git rm web/src/modules/projects/OpenRepoDialog.vue web/src/modules/projects/OpenRepoDialog.test.ts
```

Run: `pnpm -C web test`
Expected: PASS (all files).

- [ ] **Step 6: Commit**

```bash
git add -A web/src/modules/projects
git commit -m "feat(web): lay the main page out as open, template, recent"
```

---

### Task 6: Full verification

**Files:** none.

- [ ] **Step 1: Run the web suite**

Run: `pnpm -C web test`
Expected: PASS, all files, no unhandled errors.

- [ ] **Step 2: Type-check and build (includes the PWA check)**

Run: `pnpm -C web build`
Expected: `PWA check ok`.

- [ ] **Step 3: Run the Rust tests**

Run: `cargo test`
Expected: PASS, including `repo_info_exposes_the_push_time` and `repo_info_tolerates_a_missing_push_time`.

- [ ] **Step 4: Confirm a clean tree and report**

Run: `git status --short`
Expected: empty.

Report the commits, the verification output, and note that the desktop app must be rebuilt (`just build`) to see the new page.
