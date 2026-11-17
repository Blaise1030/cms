---
version: alpha
name: Acme
description: Plan your team's work on one page.
colors:
  surface: "#f6f4ef"
  surface-raised: "#fcfbf8"
  sand: "#ece7dd"
  mist: "#dde5ea"
  ridge: "#b4c0c6"
  glow: "#fdfaf2"
  ink: "#2a2e33"
  muted: "#5b6269"
  line: "#ddd7cb"
  line-strong: "#b5ae9f"
  accent: "#3b5a72"
  accent-hover: "#2e4a60"
  on-accent: "#f6f4ef"
  surface-dark: "#25272a"
  surface-raised-dark: "#2c2f32"
  sand-dark: "#2f3235"
  mist-dark: "#2f3b45"
  ridge-dark: "#4a5862"
  glow-dark: "#d8d3c6"
  ink-dark: "#eae6de"
  muted-dark: "#a9adb0"
  line-dark: "#3c3f43"
  line-strong-dark: "#5a5f64"
  accent-dark: "#a3bdd1"
  accent-hover-dark: "#bfd2e0"
  on-accent-dark: "#1d2023"
typography:
  display:
    fontFamily: "Albert Sans"
    fontSize: 3.75rem
    fontWeight: 300
    lineHeight: 1.04
    letterSpacing: -0.025em
  h1:
    fontFamily: "Albert Sans"
    fontSize: 3rem
    fontWeight: 300
    lineHeight: 1.08
    letterSpacing: -0.025em
  h2:
    fontFamily: "Albert Sans"
    fontSize: 2.375rem
    fontWeight: 300
    lineHeight: 1.15
    letterSpacing: -0.025em
  h3:
    fontFamily: "Albert Sans"
    fontSize: 1.25rem
    fontWeight: 500
    lineHeight: 1.6
    letterSpacing: -0.012em
  body-lg:
    fontFamily: "Albert Sans"
    fontSize: 1.25rem
    fontWeight: 400
    lineHeight: 1.6
  body-md:
    fontFamily: "Albert Sans"
    fontSize: 1.0625rem
    fontWeight: 400
    lineHeight: 1.7
  label:
    fontFamily: "Albert Sans"
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

A calm, Scandinavian marketing site for a planning product: birch-coloured paper, soft charcoal type, one fjord-blue accent and a lot of air. Structure comes from hairlines and alignment rather than boxes, headings are light and large, and a small landscape (sky, low sun, ridge, still water) is the recurring image. Motion is slow and quiet: the hero rises in once, later sections fade in as they arrive, and pages cross-fade.

Every value below is a CSS variable in the `@theme` block of `src/styles/global.css`, and components use only those tokens (`scripts/design-tokens.test.mjs` enforces it). Change a value there and here together.

## Colors

Light mode is the default; dark mode follows the visitor's system setting by giving the same variables warm-slate values (keys ending in `-dark` above). Components never switch colours themselves.

| Token | Utility | Light | Dark | Use |
| --- | --- | --- | --- | --- |
| `--color-surface` | `bg-surface` | `#f6f4ef` birch | `#25272a` warm slate | Page background, `theme-color` |
| `--color-surface-raised` | `bg-surface-raised` | `#fcfbf8` | `#2c2f32` | Floating card in the hero |
| `--color-sand` | `bg-sand` | `#ece7dd` | `#2f3235` | Quiet bands: testimonials, featured plan, code blocks |
| `--color-mist` | `bg-mist` | `#dde5ea` | `#2f3b45` | Sky in illustrations, closing call-to-action, notes, text selection |
| `--color-ridge` | `fill-ridge` | `#b4c0c6` | `#4a5862` | Distant hills and water in illustrations |
| `--color-glow` | `fill-glow` | `#fdfaf2` | `#d8d3c6` | Sun or moon, water highlights |
| `--color-ink` | `text-ink` | `#2a2e33` | `#eae6de` | Headings and body text |
| `--color-muted` | `text-muted` | `#5b6269` | `#a9adb0` | Supporting text, labels, input underline |
| `--color-line` | `border-line` | `#ddd7cb` | `#3c3f43` | Hairline dividers |
| `--color-line-strong` | `border-line-strong` | `#b5ae9f` | `#5a5f64` | Outline buttons, link underlines, list bullets |
| `--color-accent` | `bg-accent`, `text-accent` | `#3b5a72` fjord | `#a3bdd1` | Primary buttons, category labels, links in Markdown, focus ring |
| `--color-accent-hover` | `hover:bg-accent-hover` | `#2e4a60` | `#bfd2e0` | Primary button hover |
| `--color-on-accent` | `text-on-accent` | `#f6f4ef` | `#1d2023` | Text on the accent |

Contrast: ink on surface is 12.4:1 (light) and 12.0:1 (dark); muted is at least 4.8:1 on every surface; the accent is 6.6:1 against the page and its button text 6.6:1 (light) and 8.4:1 (dark). Hairline colours are decorative and never carry meaning alone. Rendered Markdown (`prose`) maps the typography plugin's `--tw-prose-*` variables to these tokens, so articles follow both modes without `prose-invert`.

## Typography

One family: **Albert Sans**, a variable sans whose plain, slightly geometric shapes come from Scandinavian street lettering. It is self-hosted from the `@fontsource-variable/albert-sans` package through Astro's font API (`--font-albert-sans`), so there is no third-party request; Astro preloads the upright file, uses `font-display: swap`, and generates a metric-matched Arial fallback so the swap does not shift the layout. `--font-sans` and `--font-display` both point at it, followed by `ui-sans-serif, system-ui, sans-serif`; `--font-mono` is the system monospace stack for code.

The scale steps by about 1.25 from a 17px body (`--text-*`): `xs` 0.75rem, `sm` 0.875rem, `base` 1.0625rem, `lg` 1.25rem, `xl` 1.5rem, `2xl` 1.875rem, `3xl` 2.375rem, `4xl` 3rem, `5xl` 3.75rem. Headlines are light (300) with slightly tight tracking (`tracking-display`, -0.025em) and balanced wrapping; the hero `h1` is `4xl` on phones and `5xl` from `md`, page titles likewise, section `h2`s are `3xl`. Item titles (`h3`, card titles) are medium (500) at `lg`–`xl`. Body copy is regular with generous leading (1.7) and lines kept under about 70 characters (`max-w-measure`, `max-w-prose`). The `label` utility is the only uppercase style: 0.75rem, medium, tracked 0.14em, muted. It marks dates, categories, counts and column headings, never decoration.

## Layout

The `page` utility centres content at up to 72rem (`--container-page`) with a 1.25rem gutter on phones and 2.5rem from `md`. Landing-page sections are separated by `--spacing-section` (6rem). Layouts use a 12-column grid with restrained asymmetry: a heading column of 4 and a content column of 7 starting at column 6, or a 6/6 hero. Reading pages (posts, help articles) keep text in a 42rem column (`--container-prose`). The header is sticky (4.5rem tall, `--spacing-header`) on a translucent surface with a hairline beneath; on phones the nav moves to a second row. Spacing otherwise uses Tailwind's 0.25rem step (`--spacing`).

## Elevation & Depth

Flat. Depth comes from tone (surface, sand, mist) and 1px hairlines. The single shadow, `--shadow-soft` (a 1px contact shadow plus a long, faint blur tinted from `ink`), lifts only the plan card that overlaps the hero illustration.

## Shapes

`--radius-sm` 0.25rem (focus rings, small links), `md` 0.5rem (notes, images in articles), `lg` 1rem (post images, the hero card), `xl` 1.75rem (illustration panels, pricing panels, the closing call-to-action), `full` for buttons and progress bars. Illustrations are drawn with simple SVG paths filled from the colour tokens, so they change with the theme.

## Motion

| Token | Value | Use |
| --- | --- | --- |
| `--transition-duration-quick` (`duration-quick`) | 180ms | Link and nav colour changes, the outgoing page |
| `--transition-duration-gentle` (`duration-gentle`) | 360ms | Buttons, FAQ open/close, the incoming page |
| `--transition-duration-slow` (`duration-slow`) | 800ms | Section reveal, hero rise, image zoom on card hover |
| `--ease-gentle` (`ease-gentle`) | cubic-bezier(0.22, 0.61, 0.36, 1) | Hovers and state changes |
| `--ease-settle` (`ease-settle`) | cubic-bezier(0.16, 1, 0.3, 1) | Entrances that settle into place |
| `--motion-stagger` | 120ms | Delay between `rise`, `rise-2` and `rise-3` |
| `--spacing-rise` | 0.75rem | How far entrances travel |

Page changes use Astro's `<ClientRouter />`: the old page fades out (`quick`) and the new one fades in with a small rise (`gentle`); the header does not animate. The hero and page titles rise in once on load (`--animate-rise`). Sections with `data-reveal` fade up when they reach the viewport; the script only hides sections that start below the fold, so nothing is hidden without JavaScript. Scripts that touch the page run on `astro:page-load`. Under `prefers-reduced-motion: reduce` every animation, transition, view transition and smooth scroll is switched off.

## Components

- **Header**: wordmark (sun-over-water mark plus name), muted nav links that turn `ink` on hover or when current, and a small pill `button` for the header call to action.
- **Buttons**: `button` is solid accent with `on-accent` text; `button-quiet` is a `line-strong` outline that fills with `sand` on hover. Both are pills, medium weight, padded 0.75rem × 1.5rem. Text links use the `link` utility (hairline underline that darkens on hover).
- **Hero**: label, light headline, lead paragraph, primary button with a quiet text link, then an illustration panel (`mist` sky, `ridge` hills, `accent` headlands, `sand` shore) with a raised plan card overlapping its corner; on phones the card sits under the panel.
- **Logo strip**: one muted line between hairlines: a label, then customer names.
- **Feature list**: heading column on the left; on the right, rows divided by hairlines with a medium title and muted description.
- **Testimonials**: a `sand` band with one large light quote and a second, smaller quote behind a hairline.
- **Pricing**: two panels with `xl` corners; the free plan is outlined with `line`, the recommended plan is filled with `sand` and uses the solid button.
- **FAQ**: hairline-divided `<details>` with a plus that turns into a cross when open; the same list is published as FAQPage JSON-LD.
- **Closing call to action**: a `mist` panel with a centred headline and a ridge-and-water silhouette along its base.
- **Blog cards**: image (or the token `Artwork` landscape) with `lg` corners and a slow zoom on hover, a label row (date, accent category links), medium title and muted description; the whole card is one link. The newest post is shown large beside its text.
- **Category filters**: text tabs on a hairline; the current one is `ink` with an accent underline.
- **Help**: an underlined search field with a search icon, sections in a three-column hairline grid, article lists as hairline rows, and articles with a sticky "On this page" list whose current entry gets an accent rule.
- **Footer**: wordmark and description, then four labelled link columns, then the copyright line above a hairline.

## Do's and Don'ts

Do use the tokens and the shared utilities (`page`, `label`, `link`, `button`, `button-quiet`), keep one solid button per view, prefer hairlines and space over boxes, and keep the accent for actions and small signposts. Don't add hex values, Tailwind palette classes, arbitrary colour values or `dark:` colour variants (the test fails), extra accent colours, more shadows, or entrance animations beyond `rise` and `data-reveal`.

## Voice & Tone

Calm, plain and specific. Sentences say what happens in concrete terms ("Each Friday, Acme emails everyone what they finished and what comes next") rather than promising speed or power. Headlines state the outcome in sentence case without exclamation marks; buttons say what you get ("Start a free project", "Start a 14-day trial"). Placeholder names, quotes and legal pages are marked as placeholders in the code and should be replaced with real ones.
