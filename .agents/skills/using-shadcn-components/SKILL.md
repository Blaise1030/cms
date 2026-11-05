---
name: using-shadcn-components
description: Use when building or changing UI in web/src — pages, dialogs, forms, lists, empty/loading/error states, or restyling an existing view — so the repository's shadcn-vue primitives are used before custom markup.
---

# Using shadcn components

orso's web UI is shadcn-vue on Vue 3. The primitives live in
`web/src/components/ui/` and already carry the app's spacing, focus rings,
themes and accessibility. Use them before writing markup.

## Before writing markup

1. Search `web/src/components/ui/` for the pattern.
2. Search `web/src` for an existing use of that component.
3. If it is missing, add it:

   ```sh
   pnpm -C web dlx shadcn-vue@latest add <name>
   ```

   Review the generated files before using them.
4. Only then write a custom component.

## Pick the primitive

| Need | Use |
| --- | --- |
| Action / link | `Button`, `ButtonGroup` |
| Surface / section | `Card`, `Item`, `Separator`, `Resizable` |
| List row with media, title, description, actions | `ItemGroup` → `Item` → `ItemMedia` / `ItemContent` / `ItemTitle` / `ItemDescription` / `ItemActions` |
| Form field | `Field`, `Form`, `Input`, `InputGroup`, `Textarea`, `Select`, `Combobox`, `Checkbox`, `RadioGroup`, `Switch` |
| Status / feedback | `Badge`, `Alert`, `Progress`, `Spinner`, `Skeleton`, `Empty` |
| Overlay | `Dialog`, `AlertDialog`, `Sheet`, `Drawer`, `Popover`, `DropdownMenu`, `ContextMenu`, `Tooltip` |
| Navigation | `Tabs`, `Breadcrumb`, `Command`, `Menubar`, `NavigationMenu`, `Pagination` |
| Data | `Table`, `Accordion`, `Collapsible`, `Carousel`, `Chart` |
| Chat | `Bubble`, `Message`, `MessageScroller`, `Attachment`, `Marker` |

## Rules

- Import from `@/components/ui/<name>`; do not recreate a primitive with raw
  HTML.
- Compose; do not fork. `Item` accepts `as` / `as-child` for links and buttons.
- Use `cn()` for one-off spacing or layout classes. Use semantic tokens
  (`bg-background`, `text-muted-foreground`, `border-border`, `ring-ring`); do
  not add arbitrary hex colours.
- For rows, prefer the `Item` family over hand-written flex rows. Keep titles,
  descriptions, media and actions in their slots.
- For empty, loading and error states use `Empty`, `Spinner`/`Skeleton`, and
  `Alert` instead of ad-hoc paragraphs.
- Put `data-testid` on interactive or observable nodes.
- Keep accessibility: `role`, `aria-*`, keyboard handlers, labels, and focus
  rings. A clickable `Item` needs `role`, `tabindex`, Enter and Space handling.
- When adding a shadcn component, keep its generated files as close to upstream
  as possible; put app-specific styling in the usage site.

## When custom markup is right

Custom is appropriate for the preview iframe, code editor, rich-text editor,
canvas/drag surfaces, and highly specialised visualisations. Even then, use the
primitives for the surrounding chrome, buttons, dialogs and states.

## Checklist

- [ ] I searched `web/src/components/ui` first.
- [ ] I used the existing primitive rather than duplicating its styles.
- [ ] List rows use the `Item` family.
- [ ] Empty / loading / error uses `Empty` / `Spinner` / `Skeleton` / `Alert`.
- [ ] Colours and spacing use tokens and `cn()`.
- [ ] Roles, labels, keyboard and focus states are present.
- [ ] Tests and `vue-tsc` pass.
