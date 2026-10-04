import { Node, mergeAttributes } from "@tiptap/vue-3";

/** Chip styling shared by the composer node and the sent message bubble. */
export const ELEMENT_CHIP_CLASS =
  "inline-flex items-center rounded border border-emerald-500/40 bg-emerald-500/10 px-1 font-mono text-xs leading-5 text-emerald-600 dark:text-emerald-400";

/**
 * An element picked in the preview, shown as a chip in the composer. The agent gets plain text,
 * so the chip serializes (via `getText`) to the full reference in `prompt`.
 */
export const ElementRef = Node.create({
  name: "elementRef",
  group: "inline",
  inline: true,
  atom: true,
  selectable: true,

  addAttributes() {
    return {
      label: { default: "" },
      prompt: { default: "" },
    };
  },

  parseHTML() {
    return [
      {
        tag: "span[data-element-ref]",
        getAttrs: (el) => ({
          label: el.textContent ?? "",
          prompt: el.getAttribute("data-prompt") ?? "",
        }),
      },
    ];
  },

  renderHTML({ node, HTMLAttributes }) {
    return [
      "span",
      mergeAttributes(HTMLAttributes, {
        "data-element-ref": "",
        "data-prompt": node.attrs.prompt,
        title: node.attrs.prompt,
        class: ELEMENT_CHIP_CLASS,
      }),
      `<${node.attrs.label}>`,
    ];
  },

  renderText({ node }) {
    return node.attrs.prompt;
  },
});
