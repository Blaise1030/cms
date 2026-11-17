---
version: alpha
name: Orso
description: Developer tools for content teams.
colors:
  surface: "#f7f8fa"
  surface-raised: "#ffffff"
  sand: "#eef1f6"
  mist: "#dbe7f6"
  ridge: "#9eb4cc"
  glow: "#f5f8fc"
  ink: "#0f172a"
  muted: "#475569"
  line: "#e2e8f0"
  line-strong: "#94a3b8"
  accent: "#2563eb"
  accent-hover: "#1d4ed8"
  on-accent: "#ffffff"
  surface-dark: "#0b1220"
  surface-raised-dark: "#111827"
  sand-dark: "#152033"
  mist-dark: "#1e3a5f"
  ridge-dark: "#3b82a8"
  glow-dark: "#cbd5e1"
  ink-dark: "#e2e8f0"
  muted-dark: "#94a3b8"
  line-dark: "#1e293b"
  line-strong-dark: "#475569"
  accent-dark: "#60a5fa"
  accent-hover-dark: "#93c5fd"
  on-accent-dark: "#0b1220"
typography:
  display:
    fontFamily: "Geist"
    fontSize: 3.75rem
    fontWeight: 300
    lineHeight: 1.04
    letterSpacing: -0.025em
  h1:
    fontFamily: "Geist"
    fontSize: 3rem
    fontWeight: 300
    lineHeight: 1.08
    letterSpacing: -0.025em
  h2:
    fontFamily: "Geist"
    fontSize: 2.375rem
    fontWeight: 300
    lineHeight: 1.15
    letterSpacing: -0.025em
  h3:
    fontFamily: "Geist"
    fontSize: 1.25rem
    fontWeight: 500
    lineHeight: 1.6
    letterSpacing: -0.012em
  body-lg:
    fontFamily: "Geist"
    fontSize: 1.25rem
    fontWeight: 400
    lineHeight: 1.6
  body-md:
    fontFamily: "Geist"
    fontSize: 1.0625rem
    fontWeight: 400
    lineHeight: 1.7
  label:
    fontFamily: "Geist"
    fontSize: 0.75rem
    fontWeight: 500
    lineHeight: 1.5
    letterSpacing: 0.14em
rounded:
  sm: 0.25rem
  md: 0.5rem
  lg: 1rem
  xl: 1.75rem
  full: 9999px
spacing:
  base: 0.25rem
  gutter: 1.25rem
  gutter-wide: 2.5rem
  section: 6rem
  section-wide: 9rem
  header: 4.5rem
  rise: 0.75rem
---

## Overview

A calm marketing site for Orso: cool paper surfaces, slate type, one clear blue primary and plenty of air. Structure comes from hairlines and alignment rather than boxes. Headings are light and large. Motion stays quiet: the hero rises in once, later sections fade in as they arrive, and pages cross-fade.

Every value below is a CSS variable in the `@theme` block of `src/styles/global.css`, and components use only those tokens (`scripts/design-tokens.test.mjs` enforces it). Change a value there and here together.

## Colors

Light mode is the default; dark mode follows the visitor's system setting by giving the same variables cool-slate values (keys ending in `-dark` above). Components never switch colours themselves.

| Token | Utility | Light | Dark | Use |
| --- | --- | --- | --- | --- |
| `--color-surface` | `bg-surface` | `#f7f8fa` cool paper | `#0b1220` deep navy | Page background, `theme-color` |
| `--color-surface-raised` | `bg-surface-raised` | `#ffffff` | `#111827` | Floating card in the hero |
| `--color-sand` | `bg-sand` | `#eef1f6` | `#152033` | Quiet bands: proof, notes, code blocks |
| `--color-mist` | `bg-mist` | `#dbe7f6` | `#1e3a5f` | Sky in illustrations, closing call-to-action, selection |
| `--color-ridge` | `fill-ridge` | `#9eb4cc` | `#3b82a8` | Distant hills and water in illustrations |
| `--color-glow` | `fill-glow` | `#f5f8fc` | `#cbd5e1` | Sun or moon, water highlights |
| `--color-ink` | `text-ink` | `#0f172a` | `#e2e8f0` | Headings and body text |
| `--color-muted` | `text-muted` | `#475569` | `#94a3b8` | Supporting text, labels |
| `--color-line` | `border-line` | `#e2e8f0` | `#1e293b` | Hairline dividers |
| `--color-line-strong` | `border-line-strong` | `#94a3b8` | `#475569` | Outline buttons, link underlines |
| `--color-accent` | `bg-accent`, `text-accent` | `#2563eb` blue | `#60a5fa` | Primary buttons, links, focus ring |
| `--color-accent-hover` | `hover:bg-accent-hover` | `#1d4ed8` | `#93c5fd` | Primary button hover |
| `--color-on-accent` | `text-on-accent` | `#ffffff` | `#0b1220` | Text on the accent |

Contrast: ink on surface stays well above 7:1; muted stays readable on every surface; accent button text is white on blue in light mode. Hairline colours are decorative and never carry meaning alone. Rendered Markdown (`prose`) maps the typography plugin's `--tw-prose-*` variables to these tokens.

## Typography

One family: **Geist** (Vercel), self-hosted from `@fontsource-variable/geist` through Astro's font API (`--font-geist`). Astro preloads the upright variable file, uses `font-display: swap`, and generates a metric-matched fallback. `--font-sans` and `--font-display` both point at it; `--font-mono` is the system monospace stack for code.

The scale steps by about 1.25 from a 17px body (`--text-*`): `xs` 0.75rem through `5xl` 3.75rem. Headlines are light (300) with slightly tight tracking (`tracking-display`). The `label` utility is the only uppercase style.

## Layout

The `page` utility centres content at up to 72rem with token gutters. Landing sections use `--spacing-section` (6rem). Layouts keep the 12-column asymmetry from the scaffold: heading column of 4 and content column of 7, or a 6/6 hero.

## Elevation & Depth

Flat. Depth comes from tone (surface, sand, mist) and 1px hairlines. The single shadow, `--shadow-soft`, lifts only the draft card that overlaps the hero illustration.

## Shapes

`--radius-sm` 0.25rem through `xl` 1.75rem; `full` for buttons. Illustrations use token fills so they follow the theme.

## Motion

| Token | Value | Use |
| --- | --- | --- |
| `--transition-duration-quick` | 180ms | Link and nav colour changes |
| `--transition-duration-gentle` | 360ms | Buttons, FAQ open/close |
| `--transition-duration-slow` | 800ms | Section reveal, hero rise |
| `--ease-gentle` | cubic-bezier(0.22, 0.61, 0.36, 1) | Hovers |
| `--ease-settle` | cubic-bezier(0.16, 1, 0.3, 1) | Entrances |
| `--motion-stagger` | 120ms | Delay between `rise` steps |
| `--spacing-rise` | 0.75rem | Entrance travel |

Under `prefers-reduced-motion: reduce` every animation, transition, view transition and smooth scroll is switched off.

## Components

- **Header**: wordmark plus name, muted nav, pill `button` for Get Orso.
- **Hero**: brand-forward headline, lead, primary CTA, stack secondary link, draft card over mist illustration.
- **Logo strip**: tools the product works with (Astro, Markdown, GitHub, agents).
- **Feature list**: GitHub→Orso mapping and proof points.
- **How it works**: three numbered steps ending in Publish.
- **Agent**: CMS vs Orso comparison with example prompts.
- **Proof**: quote plus maker note.
- **FAQ**: hairline `<details>` accordion with FAQPage JSON-LD.
- **Closing CTA**: mist panel with open-source promise.

## Do's and Don'ts

Do use the tokens and shared utilities (`page`, `label`, `link`, `button`, `button-quiet`). Don't add hex values, Tailwind palette classes, arbitrary colour values or `dark:` colour variants outside the token file.

## Voice & Tone

Calm, plain and specific. Say what happens: a draft, a preview, a Publish button. Headlines in sentence case; buttons name the outcome ("Get Orso", "See how it works").
