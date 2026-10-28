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
