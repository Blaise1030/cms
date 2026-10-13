---
version: alpha
name: Acme
description: Ship your product story faster with Acme.
colors:
  primary: "#171717"
  secondary: "#525252"
  tertiary: "#a3a3a3"
  neutral: "#ffffff"
typography:
  h1:
    fontFamily: "ui-sans-serif, system-ui, sans-serif"
    fontSize: 3.75rem
    fontWeight: 700
    lineHeight: 1
  h2:
    fontFamily: "ui-sans-serif, system-ui, sans-serif"
    fontSize: 1.875rem
    fontWeight: 700
    lineHeight: 1.2
  body-md:
    fontFamily: "ui-sans-serif, system-ui, sans-serif"
    fontSize: 1rem
    fontWeight: 400
    lineHeight: 1.5
  label:
    fontFamily: "ui-sans-serif, system-ui, sans-serif"
    fontSize: 1rem
    fontWeight: 600
    lineHeight: 1.5
rounded:
  sm: 0.25rem
  md: 0.375rem
  lg: 0.5rem
  full: 9999px
spacing:
  sm: 1rem
  md: 1.5rem
  lg: 4rem
---

## Overview

A clean, neutral marketing site: generous whitespace, a single dark accent and a light or dark scheme that follows the visitor's system setting. It reads as confident and unfussy so the copy does the work.

## Colors

`primary` is near-black and fills the main call-to-action button; `secondary` is a mid grey for supporting copy; `tertiary` is a light grey used for soft card borders; `neutral` is the white page background (it inverts to near-black in dark mode).

## Typography

The site uses the system sans-serif stack throughout. Headings are bold with tight tracking. The hero `h1` is responsive: 2.25rem (36px) on mobile and 3.75rem from the `sm` breakpoint up, and the `h1` token records the larger size; landing-page section `h2` headings are a fixed 1.875rem at every width. Body copy is regular weight at 1rem, and buttons and labels are semibold.

## Layout

Content sits in a centred column up to 64rem wide with 2rem side padding. Sections are separated by 4rem of vertical space, and card grids wrap automatically with a 1.5rem gap.

## Elevation & Depth

The design is flat: there are no shadows. Depth comes from thin, low-contrast borders around cards and FAQ items.

## Shapes

Buttons use 0.5rem corners and cards use slightly rounder 0.75rem corners; borders are 1px. No pill shapes are used today; `full` is there for future badges and avatars.

## Components

Primary buttons are solid `primary` with white text, secondary buttons are outlined with the current text colour, and both are padded 1.25rem by 0.625rem. Cards are bordered boxes with 1.25rem padding and the same shape is reused for features, testimonials, pricing plans and FAQ items.

## Do's and Don'ts

Do keep one solid call-to-action per section and use utility classes with `dark:` variants for every colour. Don't add shadows, extra accent colours or custom font files without updating this file first.

## Voice & Tone

Plain, friendly and benefit-led, with short sentences and no jargon. Headlines promise an outcome ("Launch your story, not your backlog") and buttons use direct verbs such as "Get started".
