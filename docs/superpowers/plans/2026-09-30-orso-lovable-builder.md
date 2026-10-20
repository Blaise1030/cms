# orso Lovable-Style Builder Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a local-first Lovable-style site builder with a Rust daemon, a Vue PWA, ACP agent sessions, live Astro preview, and git checkpoints.

**Architecture:** Start with a single `orso` Rust crate that owns auth, project lifecycle, agent sessions, preview proxying, checkpoints, and JSON APIs. Pair it with a Vue 3 + Vite PWA in `web/`, and scaffold projects from an embedded Astro marketing template under `templates/astro-marketing/`. Keep the daemon as the source of truth for state, with the web app consuming only REST and WebSocket events from `core/*`.

**Tech Stack:** Rust, axum, tokio, clap, git CLI, `agent-client-protocol` ACP adapters, Vue 3, TypeScript, Vite, pnpm, Tailwind CSS v4 (`@tailwindcss/vite`), `shadcn-vue`, `vite-plugin-pwa`, Astro, shiki, CodeMirror merge view.

**Spec:** [`docs/superpowers/specs/2026-09-29-orso-lovable-builder-design.md`](../specs/2026-09-29-orso-lovable-builder-design.md)

**Global Constraints**

- Local-first: code and agent credentials never leave the machine.
- Single Rust crate named `orso`.
- Web app is Vue 3 + TS + Vite + pnpm.
- Web app styling uses Tailwind CSS v4 (`@tailwindcss/vite`) and `shadcn-vue` (Reka UI primitives via `pnpm dlx shadcn-vue@latest`).
- `shadcn-vue` components live under `web/src/components/ui/`; shared helpers under `web/src/lib/utils.ts`.
- Template is Astro and Cloudflare Pages-ready.
- Every completed agent turn becomes a git checkpoint.
- File browsing is read-only in v1.
- Deploy execution from the UI is out of scope for v1.
- Multi-user, relay/tunneling, terminal UI, and full file editor are out of scope for v1.

---

### Task 1: Bootstrap repo skeleton + Tailwind / shadcn-vue

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`
- Create: `web/package.json`
- Create: `web/vite.config.ts`
- Create: `web/tsconfig.json`
- Create: `web/tsconfig.app.json`
- Create: `web/tsconfig.node.json`
- Create: `web/index.html`
- Create: `web/src/main.ts`
- Create: `web/src/App.vue`
- Create: `web/src/index.css`
- Create: `web/src/lib/utils.ts`
- Create: `web/components.json`
- Create: `web/src/components/ui/button/index.ts` (and sibling shadcn components added in Step 5)
- Create: `templates/astro-marketing/package.json`
- Create: `templates/astro-marketing/astro.config.mjs`
- Create: `templates/astro-marketing/src/pages/index.astro`
- Create: `justfile`

**Interfaces:**
- Consumes: none
- Produces: a buildable Rust binary stub, Vue/Vite app with Tailwind v4 + `shadcn-vue` initialized, and a minimal Astro template skeleton

- [ ] **Step 1: Write the initial failing checks**

Add the smallest smoke tests or build checks that prove the repo is wired correctly:

```bash
cargo check
pnpm -C web build
pnpm -C templates/astro-marketing build
```

- [ ] **Step 2: Scaffold Rust + Vue + Astro stubs**

Create minimal manifests and entrypoints so each command can succeed once styling is wired. Vue app uses `pnpm create vite@latest web -- --template vue-ts` (or equivalent hand-written files).

- [ ] **Step 3: Install Tailwind CSS v4**

From `web/`:

```bash
pnpm add tailwindcss @tailwindcss/vite
```

Replace `web/src/index.css` with:

```css
@import "tailwindcss";
```

Update `web/vite.config.ts`:

```ts
import path from "node:path";
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [vue(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    proxy: {
      "/api": "http://127.0.0.1:7800",
      "/ws": { target: "ws://127.0.0.1:7800", ws: true },
      "/preview": "http://127.0.0.1:7800",
    },
  },
});
```

In `web/tsconfig.json` and `web/tsconfig.app.json`, set:

```json
{
  "compilerOptions": {
    "baseUrl": ".",
    "paths": {
      "@/*": ["./src/*"]
    }
  }
}
```

Import CSS from `web/src/main.ts`:

```ts
import "./index.css";
```

- [ ] **Step 4: Initialize shadcn-vue**

From `web/`:

```bash
pnpm dlx shadcn-vue@latest init
```

Use these answers (non-interactive defaults if flags exist; otherwise interactive):

- Style: **New York**
- Base color: **Neutral**
- CSS variables: **Yes** (required for dark theme)
- Global CSS: `src/index.css`
- Import aliases: `@/components`, `@/lib/utils`, `@/composables`, `@/components/ui`

Commit the generated `web/components.json` and updated CSS theme tokens. Ensure `web/src/lib/utils.ts` exports `cn()` via `clsx` + `tailwind-merge`.

- [ ] **Step 5: Add the v1 UI primitives**

From `web/`:

```bash
pnpm dlx shadcn-vue@latest add button input textarea card badge separator \
  scroll-area dialog alert-dialog sheet dropdown-menu tooltip \
  tabs sonner resizable sidebar
```

Smoke-check one import in `App.vue`:

```vue
<script setup lang="ts">
import { Button } from "@/components/ui/button";
</script>

<template>
  <div class="dark min-h-screen bg-background text-foreground p-4">
    <Button>orso</Button>
  </div>
</template>
```

- [ ] **Step 6: Verify the bootstrap**

Run:

```bash
cargo check
pnpm -C web build
pnpm -C templates/astro-marketing build
```

Expected: all three succeed; `web/dist` includes Tailwind-styled CSS.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml src web templates justfile
git commit -m "chore: bootstrap orso with Tailwind and shadcn-vue"
```

### Task 2: Daemon foundation and shared error model

**Files:**
- Modify: `src/main.rs`
- Create: `src/state.rs`
- Create: `src/error.rs`
- Create: `src/http/mod.rs`
- Create: `src/http/health.rs`
- Create: `src/http/error.rs`

**Interfaces:**
- Consumes: the bootstrap binary from Task 1
- Produces: `orso serve`, uniform JSON errors, `/api/health`

- [ ] **Step 1: Write a failing API test**

Add an integration test that starts the daemon and asserts `GET /api/health` returns `200` with a JSON body.

- [ ] **Step 2: Implement server startup**

Wire `clap` parsing for `serve`, bind the configured port, and install auth/error middleware placeholders.

- [ ] **Step 3: Implement the error envelope**

Return `{ "error": { "code", "message" } }` for non-2xx API errors.

- [ ] **Step 4: Verify the server and test**

Run the health test and `cargo test`.

- [ ] **Step 5: Commit**

```bash
git add src
git commit -m "feat: add daemon foundation"
```

### Task 3: Project registry and scaffold creation

**Files:**
- Create: `src/projects.rs`
- Create: `src/static.rs`
- Create: `src/http/projects.rs`
- Create: `templates/astro-marketing/src/content/config.ts`
- Create: `templates/astro-marketing/src/content/site.json`
- Create: `templates/astro-marketing/src/content/pages/home.json`
- Create: `templates/astro-marketing/src/content/sections/*.mdx`
- Create: `templates/astro-marketing/AGENTS.md`

**Interfaces:**
- Consumes: daemon state and error model
- Produces: `GET /api/projects`, `POST /api/projects`, `DELETE /api/projects/{id}`

- [ ] **Step 1: Write a project creation test**

Assert that creating a project copies the embedded template, initializes git, and returns a project id and port.

- [ ] **Step 2: Implement registry storage**

Store project id, name, dir, and devserver status in daemon state.

- [ ] **Step 3: Implement scaffold copy**

Copy the embedded Astro template into the project directory and run `git init` plus the first commit.

- [ ] **Step 4: Verify project lifecycle**

Run the registry tests and confirm delete removes the project directory only after a guarded confirmation path.

- [ ] **Step 5: Commit**

```bash
git add src templates
git commit -m "feat: add project scaffolding"
```

### Task 4: Checkpoints and file browsing

**Files:**
- Create: `src/git.rs`
- Create: `src/fs.rs`
- Create: `src/http/checkpoints.rs`
- Create: `src/http/files.rs`

**Interfaces:**
- Consumes: project registry and project directory paths
- Produces: checkpoint log, restore endpoint, tree/file read endpoints

- [ ] **Step 1: Write checkpoint/restore tests**

Assert that a dirty tree creates a checkpoint commit and that restore resets files back to the captured SHA.

- [ ] **Step 2: Implement checkpointing**

Add `git add -A && git commit` after settled agent turns, and skip commits when the tree is clean.

- [ ] **Step 3: Implement read-only file APIs**

Expose tree and file reads with sandbox path canonicalization and containment checks.

- [ ] **Step 4: Verify diffs and restore**

Run the checkpoint tests and confirm unified diff output matches the commit being inspected.

- [ ] **Step 5: Commit**

```bash
git add src
git commit -m "feat: add checkpoints and file browsing"
```

### Task 5: ACP agent bridge and session lifecycle

**Files:**
- Create: `src/acp.rs`
- Create: `src/agents_config.rs`
- Create: `src/http/agent.rs`

**Interfaces:**
- Consumes: project registry, error model, checkpointing hooks
- Produces: `GET /api/agents`, `POST /api/agent/session`, `POST /api/agent/prompt`, cancel and permission endpoints, WS chat events

- [ ] **Step 1: Write a fake ACP adapter test**

Use a scripted stdio JSON-RPC stub to assert initialize, newSession, prompt, permission, and cancel flows.

- [ ] **Step 2: Implement provider registry loading**

Read `~/.config/orso/agents.json` and expose provider ids and adapter commands.

- [ ] **Step 3: Implement one live session per project**

Starting a new session terminates the previous subprocess cleanly.

- [ ] **Step 4: Implement turn streaming and history**

Persist turn summaries to `{projects-dir}/.orso/{id}/history.jsonl` and emit `chat.chunk`, `chat.tool`, `chat.done`, and `permission.request` events.

- [ ] **Step 5: Verify permission handling**

Assert command execution requires Allow/Deny and file edits are auto-approved.

- [ ] **Step 6: Commit**

```bash
git add src
git commit -m "feat: add ACP agent bridge"
```

### Task 6: Dev server supervision and preview proxy

**Files:**
- Create: `src/devserver.rs`
- Create: `src/preview.rs`
- Create: `src/http/devserver.rs`

**Interfaces:**
- Consumes: project registry and event broadcaster
- Produces: supervised `npm run dev`, `/preview/{id}` reverse proxy, crash restart, status events

- [ ] **Step 1: Write a proxy integration test**

Assert HTTP and WebSocket traffic are forwarded through the preview prefix without rewriting HTML.

- [ ] **Step 2: Implement dev server spawn and readiness detection**

Allocate a free port, set `PUBLIC_BASE=/preview/{id}/`, and mark the server ready from stdout.

- [ ] **Step 3: Implement reverse proxy passthrough**

Forward `/preview/{id}/*` and WS upgrades to the correct local dev server.

- [ ] **Step 4: Implement bounded crash restart**

Retry up to three times with backoff and publish `devserver.status`.

- [ ] **Step 5: Commit**

```bash
git add src
git commit -m "feat: add preview proxy and devserver supervision"
```

### Task 7: Web app core, sockets, and module shell

**Files:**
- Create: `web/src/core/api.ts`
- Create: `web/src/core/socket.ts`
- Create: `web/src/core/types.ts`
- Create: `web/src/app/Shell.vue`
- Create: `web/src/app/DaemonUnreachable.vue`
- Modify: `web/src/App.vue`
- Modify: `web/src/index.css` (ensure `.dark` + CSS variable theme from shadcn init)

**Interfaces:**
- Consumes: daemon REST/WS contract; `shadcn-vue` primitives from Task 1 (`Button`, `Card`, `Resizable`, `Sonner`, `Sidebar`)
- Produces: typed client primitives and the dark split-pane shell for later modules

- [ ] **Step 1: Write API/client tests**

Add Vitest coverage for error mapping, socket reconnects, and event decoding.

- [ ] **Step 2: Implement the core client**

Centralize daemon-origin handling, auth token propagation, and event parsing in `core/*`. Modules must import only `core/*`, `@/components/ui/*`, and their own files.

- [ ] **Step 3: Build the shell with shadcn layout primitives**

Render PREVIEW LEFT · CHAT RIGHT using `ResizablePanelGroup` / `ResizablePanel` / `ResizableHandle`. Mount `Toaster` from `sonner`. Default root to `class="dark"`. Show `DaemonUnreachable` when the socket cannot connect.

Example shell skeleton:

```vue
<script setup lang="ts">
import { Toaster } from "@/components/ui/sonner";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
</script>

<template>
  <div class="dark h-screen bg-background text-foreground">
    <ResizablePanelGroup direction="horizontal" class="h-full">
      <ResizablePanel :default-size="60"><!-- preview slot --></ResizablePanel>
      <ResizableHandle with-handle />
      <ResizablePanel :default-size="40"><!-- chat slot --></ResizablePanel>
    </ResizablePanelGroup>
    <Toaster />
  </div>
</template>
```

- [ ] **Step 4: Verify web build**

Run `pnpm -C web test` and `pnpm -C web build`.

- [ ] **Step 5: Commit**

```bash
git add web
git commit -m "feat: add web app shell and daemon client"
```

### Task 8: Projects, chat, preview, checkpoints, and files UI

**Files:**
- Create: `web/src/modules/projects/*`
- Create: `web/src/modules/chat/*`
- Create: `web/src/modules/preview/*`
- Create: `web/src/modules/checkpoints/*`
- Create: `web/src/modules/files/*`

**Interfaces:**
- Consumes: core client; `shadcn-vue` primitives (`Button`, `Input`, `Textarea`, `Card`, `Badge`, `Dialog`, `AlertDialog`, `ScrollArea`, `Tabs`, `DropdownMenu`, `Tooltip`, `Sheet`)
- Produces: the user-facing Lovable-style workflow

- [ ] **Step 1: Write store and component tests**

Cover project list loading, new project creation, chat streaming, permission prompts, checkpoint restore, and file browsing.

- [ ] **Step 2: Implement project management**

Load projects, create projects (dialog + input), and show status with `Badge`. Delete uses `AlertDialog`.

- [ ] **Step 3: Implement chat + permissions**

Render streamed assistant text in a `ScrollArea`, tool cards with `Card`/`Badge`, allow/deny via `AlertDialog` or inline `Button`s, stop with a destructive `Button`.

- [ ] **Step 4: Implement preview and checkpoints**

Embed the proxied preview iframe; device size toggles as `Tabs` or button group. Checkpoint timeline in a `ScrollArea`; restore confirm via `AlertDialog`; diffs in `@codemirror/merge`.

- [ ] **Step 5: Implement read-only file browsing**

Tree in a `ScrollArea`/`Sheet`; file body via shiki. No editing controls.

- [ ] **Step 6: Commit**

```bash
git add web
git commit -m "feat: add builder workflow UI"
```

### Task 9: PWA, hosting, and embedded build

**Files:**
- Update: `web/vite.config.ts`
- Create: `web/public/manifest.webmanifest`
- Create: `web/public/icons/*`
- Update: `src/static.rs`
- Update: `Cargo.toml`
- Create: `justfile`

**Interfaces:**
- Consumes: completed web app
- Produces: installable PWA, embedded assets, and a repeatable dev/build workflow

- [ ] **Step 1: Write PWA build checks**

Verify the app shell is precached and offline errors surface as daemon-unreachable states.

- [ ] **Step 2: Add vite-plugin-pwa**

Configure hosted-origin precaching only, with no live daemon content cached.

- [ ] **Step 3: Embed the built web app**

Serve the built SPA and scaffold template assets from Rust using rust-embed.

- [ ] **Step 4: Add top-level commands**

Wire `just dev`, `just build`, and `just test` to the daemon, web app, and template.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml justfile src web
git commit -m "feat: add PWA packaging and embedded hosting"
```

### Task 10: End-to-end verification and hardening

**Files:**
- Update: the files touched across Tasks 1 to 9

**Interfaces:**
- Consumes: all previous tasks
- Produces: a release-ready v1 with passing tests and spec coverage

- [ ] **Step 1: Run the full test matrix**

Execute Rust tests, web tests, template builds, and at least one manual create-agent-preview-checkpoint flow.

- [ ] **Step 2: Fix any integration gaps**

Resolve mismatches between the daemon contract and the web client, then rerun the affected tests.

- [ ] **Step 3: Verify spec coverage**

Confirm every spec section is implemented or explicitly deferred by v1 scope.

- [ ] **Step 4: Commit the integration pass**

```bash
git add -A
git commit -m "test: verify orso v1 end to end"
```

