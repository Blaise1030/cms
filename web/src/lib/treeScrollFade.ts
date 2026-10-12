/**
 * `unsafeCSS` for a Pierre file tree: fades the top edge once scrolled and the bottom edge
 * while more rows remain. Driven by the scroll position, so trees that fit show no fade.
 * `--tree-fade-top`/`--tree-fade-bottom` are registered in index.css, since `@property`
 * is ignored inside the tree's shadow root.
 */
export const TREE_SCROLL_FADE_CSS = `
@supports (animation-timeline: scroll()) {
  [data-file-tree-virtualized-scroll="true"] {
    mask-image: linear-gradient(to bottom, transparent, #000 var(--tree-fade-top), #000 calc(100% - var(--tree-fade-bottom)), transparent);
    animation: tree-scroll-fade linear both;
    animation-timeline: scroll(self);
  }
  @keyframes tree-scroll-fade {
    0% { --tree-fade-top: 0px; --tree-fade-bottom: 24px; }
    10%, 90% { --tree-fade-top: 24px; --tree-fade-bottom: 24px; }
    100% { --tree-fade-top: 24px; --tree-fade-bottom: 0px; }
  }
}`;
