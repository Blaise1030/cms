---
title: Change colours and fonts
description: Adjust your site's look in the Design tab.
category: editing-content
order: 3
---

The **Design** tab holds settings that apply to the whole site, such as colours, fonts and spacing. They are stored in `DESIGN.md` at the root of your repo.

## Change a setting

1. Open the **Design** tab.
2. Change a colour, font or other value.
3. Check the result in the **Preview**, in both light and dark mode if your site has both.

Like every edit, design changes go onto your current change and only reach the live site when you publish.

## Keep things readable

- Make sure text has enough contrast against its background, especially for small text and buttons.
- Use one or two typefaces at most.

## Bigger redesigns

For larger changes, such as a new layout or a new section style, describe what you want to the assistant. For example: "make the header sticky and use the accent colour for the active link".

## Sites with Tailwind

If your site uses Tailwind CSS v4, the same values also live in an `@theme` block in your main CSS file. orso asks the assistant to keep the two in sync when you change a design setting.
