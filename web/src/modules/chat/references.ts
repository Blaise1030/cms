export type Segment =
  | { kind: "text"; text: string }
  | { kind: "element"; label: string; full: string; screenshot: boolean };

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
    out.push({ kind: "element", label: label(m[1]!), full: m[0], screenshot: m[2] !== undefined });
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push({ kind: "text", text: text.slice(last) });
  return out;
}

/** A chat title split for a tab: picked-element labels first, then the words. */
export interface TitleParts {
  elements: string[];
  text: string;
}

/**
 * Chat titles are the first message cut to a length, so an element reference can be cut
 * mid-tag (`[element <div class="rel…`). Complete ones and a cut-off one both become labels.
 */
export function titleParts(title: string): TitleParts {
  const elements: string[] = [];
  const words: string[] = [];
  for (const seg of segmentReferences(title)) {
    if (seg.kind === "element") {
      elements.push(seg.label);
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
  }
  return { elements, text: words.join(" ").replace(/\s+/g, " ").trim() };
}
