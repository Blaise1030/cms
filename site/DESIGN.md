---
version: alpha
name: Project Orso
description: Developer tools for content teams.
colors:
  surface: "#fafaf9"
  surface-raised: "#ffffff"
  sand: "#f4f4f5"
  mist: "#eaeae7"
  ridge: "#c4c4c0"
  glow: "#fafaf9"
  ink: "#18181b"
  muted: "#52525b"
  line: "#e4e4e7"
  line-strong: "#a1a1aa"
  accent: "#2563eb"
  accent-hover: "#1d4ed8"
  on-accent: "#ffffff"
  night: "#18181b"
  night-raised: "#242427"
  night-line: "#3f3f46"
  on-night: "#fafafa"
  on-night-muted: "#a1a1aa"
  status-draft: "#b45309"
  status-review: "#2563eb"
  status-live: "#15803d"
  status-danger: "#b91c1c"
  surface-dark: "#0c0c0d"
  surface-raised-dark: "#141416"
  sand-dark: "#18181a"
  mist-dark: "#222225"
  ink-dark: "#f4f4f5"
  muted-dark: "#a1a1aa"
  line-dark: "#27272a"
  line-strong-dark: "#52525b"
  accent-dark: "#60a5fa"
  accent-hover-dark: "#93c5fd"
  night-dark: "#1b1b1e"
  status-draft-dark: "#fbbf24"
  status-review-dark: "#60a5fa"
  status-live-dark: "#4ade80"
  status-danger-dark: "#f87171"
typography:
  display:
    fontFamily: "Geist"
    fontSize: clamp(2.75rem, 6vw, 4.75rem)
    fontWeight: 500
    lineHeight: 1.02
    letterSpacing: -0.035em
  h1:
    fontFamily: "Geist"
    fontSize: 3rem
    fontWeight: 500
    lineHeight: 1.08
    letterSpacing: -0.035em
  h2:
    fontFamily: "Geist"
    fontSize: 2.25rem
    fontWeight: 500
    lineHeight: 1.15
    letterSpacing: -0.035em
  h3:
    fontFamily: "Geist"
    fontSize: 1.4375rem
    fontWeight: 500
    lineHeight: 1.35
    letterSpacing: -0.015em
  body-lg:
    fontFamily: "Geist"
    fontSize: 1.1875rem
    fontWeight: 400
    lineHeight: 1.55
  body-md:
    fontFamily: "Geist"
    fontSize: 1rem
    fontWeight: 400
    lineHeight: 1.65
  label:
    fontFamily: "Geist"
    fontSize: 0.75rem
    fontWeight: 500
    lineHeight: 1.5
    letterSpacing: 0.02em
rounded:
  sm: 0.25rem
  md: 0.5rem
  lg: 0.75rem
  xl: 1rem
  full: 9999px
spacing:
  base: 0.25rem
  gutter: 1.25rem
  gutter-wide: 2.5rem
  section: 4.5rem
  section-wide: 7rem
  header: 4rem
  rise: 0.75rem
---

## Overview

Project Orso is a working desk for a content team, so the site looks like a well-set technical document rather than a brochure. Paper and ink, white space, hairlines, and the product itself as the only illustration. Type is large and tightly set; copy is small and plain; colour is saved for status.

Every value below is a CSS variable in the `@theme` block of `src/styles/global.css`, and components use only those tokens (`scripts/design-tokens.test.mjs` enforces it). Change a value there and here together.

## Colors

Light mode is the default; dark mode follows the visitor's system setting by giving the same variables neutral values (keys ending in `-dark` above). Components never switch colours themselves.

| Token | Utility | Light | Dark | Use |
| --- | --- | --- | --- | --- |
| `--color-surface` | `bg-surface` | `#fafaf9` proof paper | `#0c0c0d` | Page background, `theme-color` |
| `--color-surface-raised` | `bg-surface-raised` | `#ffffff` | `#141416` | Product panels, inputs, mini-site paper |
| `--color-sand` | `bg-sand` | `#f4f4f5` | `#18181a` | Quiet bands, rails, inset panels |
| `--color-mist` | `bg-mist` | `#eaeae7` | `#222225` | Selection, legal note, subtle blocks |
| `--color-ridge` | `fill-ridge` | `#c4c4c0` | `#3f3f46` | Muted illustration strokes |
| `--color-glow` | `fill-glow` | `#fafaf9` | `#e4e4e7` | Illustration highlights |
| `--color-ink` | `text-ink` | `#18181b` | `#f4f4f5` | Headings, body, primary buttons |
| `--color-muted` | `text-muted` | `#52525b` | `#a1a1aa` | Supporting text, labels, metadata |
| `--color-line` | `border-line` | `#e4e4e7` | `#27272a` | Hairline dividers and panel edges |
| `--color-line-strong` | `border-line-strong` | `#a1a1aa` | `#52525b` | Outline buttons, link underlines |
| `--color-accent` | `bg-accent`, `text-accent`, `bg-primary` | `#2663eb` | `#60a5fa` | App blue primary (shadcn `--primary`), focus ring, links |
| `--color-accent-hover` | `hover:bg-accent-hover` | `#1d4ed8` | `#93c5fd` | Accent hover |
| `--color-on-accent` | `text-on-accent` | `#ffffff` | `#0c0c0d` | Text on accent |
| `--color-night` | `bg-night` | `#18181b` | `#1b1b1e` | The proof band and code-style surfaces |
| `--color-night-raised` | `bg-night-raised` | `#242427` | `#232326` | Raised detail inside the proof band |
| `--color-night-line` | `border-night-line` | `#3f3f46` | `#3f3f46` | Hairlines on night |
| `--color-on-night` | `text-on-night` | `#fafafa` | `#fafafa` | Type on night |
| `--color-on-night-muted` | `text-on-night-muted` | `#a1a1aa` | `#a1a1aa` | Supporting type on night |
| `--color-status-draft` | `text-status-draft` | `#b45309` | `#fbbf24` | Waiting for a person |
| `--color-status-review` | `text-status-review` | `#2563eb` | `#60a5fa` | Preview and review ready |
| `--color-status-live` | `text-status-live` | `#15803d` | `#4ade80` | Published, passing checks |
| `--color-status-danger` | `text-status-danger` | `#b91c1c` | `#f87171` | Deletions, failures |

Status colours are information, never decoration. They appear in small dots, pills and diff numbers; a page should still read correctly in monochrome. Ink on surface stays well above 7:1; muted stays readable on every surface. Rendered Markdown (`prose`) maps the typography plugin's `--tw-prose-*` variables to these tokens.

## Typography

One family: **Geist** (Vercel), self-hosted from `@fontsource-variable/geist` through Astro's font API (`--font-geist`). Astro preloads the upright variable file, uses `font-display: swap`, and generates a metric-matched fallback. `--font-sans` and `--font-display` both point at it; `--font-mono` is the system monospace stack for repo paths, hashes and field values.

The scale steps by about 1.22 from a 16px body (`--text-*`): `xs` 0.75rem through `5xl` 3.75rem, plus a responsive `display` step (`clamp(2.75rem, 6vw, 4.75rem)`) for the home hero only. Display type is weight 500 with tight tracking (`tracking-display`, -0.035em); body copy is 400. `label` is a small sentence-case supporting label, never uppercase.

## Layout

The home page follows Medusa's section map with Orso copy and tokens. The `page` utility spans the full viewport with token gutters, so sections keep their side padding. Sections that look best at the edge omit `page` and keep their copy in a nested `page` wrapper; the hero's product rail and app window break out that way. Interiors cap themselves where the copy needs a measure (`max-w-prose`, `max-w-measure`, `max-w-3xl`). `--container-page` (80rem) stays available for sections that want a narrower centred column. Landing sections are full-bleed bands; interiors use the 12-column grid where needed.

Order: hero (left-aligned headline, lead, dual CTAs, five product tabs switching one panel) → bordered logo grid → horizontal ops carousel → three feature cards → agent product band → night proof → FAQ → centred closing CTA. Sections use `--spacing-section` (4.5rem); the proof band may use `--spacing-section-wide` (7rem).

The hero tab rail is five equal columns of title and one-line description, hairline dividers, a 2px ink underline on the active tab, and one large product panel underneath. On narrow screens the rail wraps to two columns.

## Elevation & Depth

Depth comes from tone (surface, sand, night), 1px hairlines, and a black-tinted raised shadow. `--shadow-soft` is a light lift; `--shadow-raised` is the stronger lift used on the hero product frame and the agent window. Carousel cards stay flat with a hairline ring, and shadows never tint with the text colour so dark mode shows a shadow rather than a glow.

## Art

Public-domain Renaissance paintings sit behind the product previews, so the app windows read as objects in front of a picture. The hero uses Patinir's *Landscape with the Flight into Egypt* inside the active tab panel, not as a band around the tabs; the three feature cards use Bruegel, Giorgione and Altdorfer; the ops carousel cycles all five; the agent band uses Leonardo's *The Virgin of the Rocks*. Each image is cropped to fill its frame, covered by a flat `bg-night` scrim (20–30%) so the interface keeps contrast, and never used for meaning. Files, sources and licences are in `public/paintings/CREDITS.md`; the ids live in `src/lib/paintings.ts` and render through `Painting.astro`.


## Shapes

Buttons use `--radius-md` (0.5rem) and are compact, never pills. Product panels, carousel cards and the tab frame use `--radius-md`; large bands and the three feature cards are square-edged frames. `full` is reserved for status pills, tags, avatars and carousel arrows.

## Motion

| Token | Value | Use |
| --- | --- | --- |
| `--transition-duration-quick` | 160ms | Link and nav colour changes |
| `--transition-duration-gentle` | 320ms | Buttons, tab underline, FAQ open/close |
| `--transition-duration-slow` | 700ms | Hero words, section entrances |
| `--ease-gentle` | cubic-bezier(0.22, 0.61, 0.36, 1) | Hovers |
| `--ease-settle` | cubic-bezier(0.16, 1, 0.3, 1) | The hero sequence |
| `--motion-stagger` | 45ms | Delay between hero words |
| `--spacing-rise` | 0.75rem | Entrance travel |

There is one orchestrated page-load moment: the hero headline arrives word by word with a short blur, followed by the lead, actions and product panel. Nothing fades in on scroll. Motion after load answers an action (a tab click, an FAQ opening, a page change). Under `prefers-reduced-motion: reduce` every animation, transition, view transition and smooth scroll is switched off.

## Components

- **Header**: wordmark plus name, plain muted nav, outline secondary CTA and ink primary CTA. No pill buttons.
- **Hero**: left-aligned display headline, lead, dual CTAs, then five product tabs (Content, Preview, Publish, Agent, History) that switch one large product panel underneath. The tab rail sits in a framed surface, and the active panel floats on a painted stage inside the content area.
- **Logo grid**: bordered cells for Astro, Markdown, GitHub, Claude, Cursor, Codex, plus a one-line git vocabulary note.
- **Ops carousel**: “Handle content work at scale” with prev/next controls and horizontal product slides (collections, preview, publish, agent, history), each floating on its own painting.
- **Feature cards**: three equal frames — Drafts and checkpoints, Previews and review, Agent in plain English — each holding a mini preview over a painting.
- **Agent band**: wide framed agent transcript mock over a painting, with a draft summary.
- **Proof**: the one night band — Lee Robinson's quote and a note from the maker.
- **FAQ**: hairline `<details>` accordion with FAQPage JSON-LD.
- **Closing CTA**: left-aligned headline and supporting copy on a 12-column split, one action, and three plain facts in a hairline-divided strip.

## Do's and Don'ts

Do use the tokens and shared utilities (`page`, `label`, `link`, `button`, `button-quiet`, `chip`). Keep every new product mock monochrome apart from the four status colours. Don't add hex values, Tailwind palette classes, arbitrary colour values or `dark:` colour variants outside the token file. Don't add decorative gradients, stock photography, rounded feature cards, shadows on every card, or scroll-triggered reveals.

## Voice & Tone

Calm, plain and specific. Say what happens: a draft, a preview, a Publish button. Headlines in sentence case and outcome-led; buttons name the outcome ("Get Orso", "See what it does"). No cleverness that hides the point.
