export type Segment =
  | { kind: "text"; text: string }
  /** A badge: a picked page element, or text selected in the content editor. */
  | { kind: "element"; label: string; full: string; screenshot: boolean; icon: string };

const ELEMENT_ICON = "i-hugeicons-cursor-pointer-01";
const SELECTION_ICON = "i-hugeicons-text-selection";
/** Longest quoted selection sent to the agent; the file path lets it read the rest. */
const SELECTION_MAX = 600;

/** `[selection in content/blog/a.md: "…"]`: quotes and backslashes in the text are escaped. */
const SELECTION = /\[selection in ([^:\]\n]+): "((?:[^"\\]|\\.)*)"\]/g;

/** The reference text for a content-editor selection. */
export function selectionPrompt(path: string, text: string): string {
  const clipped = text.length > SELECTION_MAX ? `${text.slice(0, SELECTION_MAX)}…` : text;
  return `[selection in ${path}: "${clipped.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"]`;
}

/** `a.md “The designer shares…”`: the file name and the selection's first words. */
function selectionLabel(path: string, quoted: string): string {
  const file = path.split("/").pop() || path;
  const words = quoted.replace(/\\(.)/g, "$1").replace(/\s+/g, " ").trim();
  return `${file} “${words.length > 24 ? `${words.slice(0, 24)}…` : words}”`;
}

/** `elementPrompt`'s output, optionally followed by its screenshot path (spaces escaped). */
const ELEMENT = /\[element (<[^>]*>)[^\]]*\](?: ((?:\\ |\S)+\/attachments\/(?:\\ |\S)+))?/g;

/** `section#hero`, `h1.title`, or just `p`, from an element's opening tag. */
function label(tag: string): string {
  const name = /^<([\w-]+)/.exec(tag)?.[1] ?? "element";
  const id = /\sid="([^"]+)"/.exec(tag)?.[1];
  if (id) return `${name}#${id}`;
  const cls = /\sclass="\s*([^"\s]+)/.exec(tag)?.[1];
  return cls ? `${name}.${cls}` : name;
}

/** A short badge label for a picked-element prompt. */
export function referenceLabel(text: string): string {
  const [first] = segmentReferences(text);
  return first?.kind === "element" ? first.label : "element";
}

/** Split sent text into plain runs and picked-element references, for compact display. */
export function segmentReferences(text: string): Segment[] {
  const out: Segment[] = [];
  let last = 0;
  for (const m of text.matchAll(ELEMENT)) {
    if (m.index > last) out.push({ kind: "text", text: text.slice(last, m.index) });
    out.push({ kind: "element", label: label(m[1]!), full: m[0], screenshot: m[2] !== undefined, icon: ELEMENT_ICON });
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push({ kind: "text", text: text.slice(last) });
  return out.flatMap((seg) => (seg.kind === "text" ? splitSelections(seg.text) : [seg]));
}

function splitSelections(text: string): Segment[] {
  const out: Segment[] = [];
  let last = 0;
  for (const m of text.matchAll(SELECTION)) {
    if (m.index > last) out.push({ kind: "text", text: text.slice(last, m.index) });
    out.push({ kind: "element", label: selectionLabel(m[1]!, m[2]!), full: m[0], screenshot: false, icon: SELECTION_ICON });
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push({ kind: "text", text: text.slice(last) });
  return out;
}

/** A chat title split for a tab: picked-element labels first, then the words. */
export interface TitleParts {
  elements: string[];
  /** The icon for each of `elements`: a picked element or an editor selection. */
  icons: string[];
  text: string;
}

/**
 * Chat titles are the first message cut to a length, so an element reference can be cut
 * mid-tag (`[element <div class="rel…`). Complete ones and a cut-off one both become labels.
 */
export function titleParts(title: string): TitleParts {
  const elements: string[] = [];
  const icons: string[] = [];
  const words: string[] = [];
  for (const seg of segmentReferences(title)) {
    if (seg.kind === "element") {
      elements.push(seg.label);
      icons.push(seg.icon);
      continue;
    }
    const sel = /\[selection in ([^:\]\n]+)(?::.*)?$/.exec(seg.text);
    if (sel) {
      // A title cut inside a selection reference: keep the file it came from.
      words.push(seg.text.slice(0, sel.index));
      elements.push(sel[1]!.split("/").pop() || sel[1]!);
      icons.push(SELECTION_ICON);
      continue;
    }
    const cut = seg.text.indexOf("[element <");
    if (cut < 0) {
      words.push(seg.text);
      continue;
    }
    words.push(seg.text.slice(0, cut));
    // Close the cut tag at its last whole attribute so `label` can read it.
    const tag = seg.text.slice(cut + "[element ".length).replace(/\s+[\w-]+(="[^"]*)?…?$/, "");
    elements.push(label(`${tag}>`));
    icons.push(ELEMENT_ICON);
  }
  return { elements, icons, text: words.join(" ").replace(/\s+/g, " ").trim() };
}

/** A chat title read aloud or in a tooltip: `div.page Move the menu`, not raw markup. */
export function readableTitle(title: string): string {
  const { elements, text } = titleParts(title);
  return [elements[0], text].filter(Boolean).join(" ") || title;
}
