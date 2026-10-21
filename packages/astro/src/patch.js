import { isDeepStrictEqual } from "node:util";

import jsyaml from "js-yaml";
import { applyEdits, findNodeAtLocation, modify, parseTree } from "jsonc-parser";
import { Document, isMap, isScalar, isSeq, parseDocument, visit } from "yaml";

/** Complete-candidate validation failed; carries Zod-style issue paths. */
export class SchemaIssuesError extends Error {
  constructor(issues) {
    super(JSON.stringify(issues));
    this.name = "SchemaIssuesError";
    this.issues = issues;
  }
}

/** The source cannot be patched without losing structure the editor preserves. */
export class PatchError extends Error {
  constructor(message) {
    super(message);
    this.name = "PatchError";
  }
}

/**
 * Leaf edits between two values. An array whose length changes yields its paired
 * element edits plus one `splice` (at/remove/insert) so list formatting survives.
 * At most one of remove and insert is nonzero.
 */
function changes(old, next, path = [], result = []) {
  if (isDeepStrictEqual(old, next)) return result;
  if (old && next && typeof old === "object" && typeof next === "object" && !Array.isArray(old) && !Array.isArray(next)) {
    for (const key of new Set([...Object.keys(old), ...Object.keys(next)])) changes(old[key], next[key], [...path, key], result);
  } else if (Array.isArray(old) && Array.isArray(next)) {
    const shortest = Math.min(old.length, next.length);
    let prefix = 0, suffix = 0;
    if (old.length !== next.length) {
      while (prefix < shortest && isDeepStrictEqual(old[prefix], next[prefix])) prefix++;
      while (suffix < shortest - prefix && isDeepStrictEqual(old[old.length - 1 - suffix], next[next.length - 1 - suffix])) suffix++;
    }
    const oldMiddle = old.length - prefix - suffix, newMiddle = next.length - prefix - suffix, paired = Math.min(oldMiddle, newMiddle);
    for (let index = prefix; index < prefix + paired; index++) changes(old[index], next[index], [...path, index], result);
    if (oldMiddle !== newMiddle) result.push({ path, value: next, splice: { at: prefix + paired, remove: oldMiddle - paired, insert: next.slice(prefix + paired, prefix + newMiddle) } });
  } else result.push({ path, value: next });
  return result;
}

/** Validate the complete merged candidate; no source edits happen before this. */
function validateCandidate(schema, fields) {
  const result = schema.safeParse(fields);
  if (!result.success) throw new SchemaIssuesError(result.error.issues.map(({ path, message }) => ({ path, message })));
}

/** Merge a partial top-level update; `unset` names top-level keys to remove. */
function candidate(previous, fields, unset) {
  const next = { ...previous, ...fields };
  for (const key of unset) delete next[key];
  return next;
}

/** Serialize like a sibling element: spacing inside one-line objects, indentation for multi-line ones. */
function jsonElement(value, sibling, lineIndent, unit, eol) {
  if (sibling === undefined || value === null || typeof value !== "object") return JSON.stringify(value);
  if (/[\r\n]/.test(sibling)) return JSON.stringify(value, null, unit).replace(/\n/g, eol + lineIndent);
  const pad = /^[[{]\s/.test(sibling) ? " " : "", colon = /"\s*:\s/.test(sibling) ? ": " : ":", comma = /,\s/.test(sibling) ? ", " : ",";
  const inline = (item) => {
    if (item === null || typeof item !== "object") return JSON.stringify(item);
    const parts = Array.isArray(item) ? item.map(inline) : Object.entries(item).filter(([, v]) => v !== undefined).map(([k, v]) => JSON.stringify(k) + colon + inline(v));
    const [open, close] = Array.isArray(item) ? ["[", "]"] : ["{", "}"];
    return parts.length ? open + pad + parts.join(comma) + pad + close : open + close;
  };
  return inline(value);
}

/** Insert or remove array elements in place, reusing the list's own separators. */
function spliceList(text, items, open, close, { at, remove, insert }, element) {
  const separator = items.length >= 2 ? text.slice(items[0].end, items[1].start) : items.length === 1 && /\n/.test(text.slice(open, items[0].start)) ? "," + text.slice(open, items[0].start) : ", ";
  if (remove) {
    if (at + remove < items.length) return text.slice(0, items[at].start) + text.slice(items[at + remove].start);
    if (at > 0) return text.slice(0, items[at - 1].end) + text.slice(items[at + remove - 1].end);
    return text.slice(0, open) + text.slice(close);
  }
  const values = insert.map((value) => element(value, items[0])).join(separator);
  if (at < items.length) return text.slice(0, items[at].start) + values + separator + text.slice(items[at].start);
  if (at > 0) return text.slice(0, items[at - 1].end) + separator + values + text.slice(items[at - 1].end);
  return text.slice(0, open) + values + text.slice(close);
}

function patchJson(source, fields, unset, schema) {
  let previous;
  try {
    previous = JSON.parse(source);
  } catch (error) {
    throw new PatchError(`Invalid JSON: ${error.message}`);
  }
  if (!previous || Array.isArray(previous) || typeof previous !== "object") throw new PatchError("Expected JSON object");
  const next = candidate(previous, fields, unset);
  validateCandidate(schema, next);
  const unit = /^([ \t]+)\S/m.exec(source)?.[1] ?? "  ";
  let result = source;
  for (const { path, value, splice } of changes(previous, next)) {
    const node = findNodeAtLocation(parseTree(result), path);
    if (splice) {
      if (node?.type !== "array") throw new PatchError("Cannot safely change this JSON list; edit this field in Code");
      const items = (node.children ?? []).map((child) => ({ start: child.offset, end: child.offset + child.length }));
      const lineIndent = items.length ? result.slice(result.lastIndexOf("\n", items[0].start - 1) + 1, items[0].start) : "";
      const element = (value, sibling) => jsonElement(value, sibling && result.slice(sibling.start, sibling.end), /^\s*$/.test(lineIndent) ? lineIndent : "", unit, result.includes("\r\n") ? "\r\n" : "\n");
      result = spliceList(result, items, node.offset + 1, node.offset + node.length - 1, splice, element);
    } else if (node && value !== undefined) result = result.slice(0, node.offset) + JSON.stringify(value) + result.slice(node.offset + node.length);
    else result = applyEdits(result, modify(result, path, value, {}));
  }
  return result;
}

// Inspect syntax tokens rather than '#' text, which may be part of a scalar value.
function yamlComments(token, ranges = []) {
  if (!token || typeof token !== "object") return ranges;
  if (token.type === "comment") ranges.push({ start: token.offset, end: token.offset + token.source.length });
  for (const child of Object.values(token)) {
    if (Array.isArray(child)) child.forEach((value) => yamlComments(value, ranges));
    else if (child && typeof child === "object") yamlComments(child, ranges);
  }
  return ranges;
}

/** Astro reads frontmatter with js-yaml; a plain scalar is safe only if js-yaml reads it back unchanged. */
function plainIsString(value) {
  try {
    return jsyaml.load(value) === value;
  } catch {
    return false;
  }
}

function yamlText(value, options) {
  const document = new Document(value);
  visit(document, {
    Scalar(key, node) {
      if (key !== "key" && typeof node.value === "string" && !plainIsString(node.value)) node.type = "QUOTE_DOUBLE";
    },
  });
  return document.toString({ lineWidth: 0, ...options }).replace(/\r?\n$/, "");
}

const yamlFlow = (value) => yamlText(value, { collectionStyle: "flow" });
const lineStart = (source, offset) => source.lastIndexOf("\n", offset - 1) + 1;

function spliceYaml(source, document, path, splice, eol) {
  const sequence = document.getIn(path, true);
  const refuse = () => {
    throw new PatchError("Cannot safely change this YAML list; edit this field in Code");
  };
  if (!isSeq(sequence) || !sequence.range) refuse();
  if (sequence.flow) {
    const [open, close] = [sequence.range[0], sequence.range[1] - 1];
    if (source[open] !== "[" || source[close] !== "]") refuse();
    const items = sequence.items.map((item) => ({ start: item.range[0], end: item.range[1] }));
    // Serialize inside a flow sequence so commas and brackets are quoted for that context.
    const replaced = spliceList(source, items, open + 1, close, splice, (value) => yamlFlow([value]).slice(1, -1).trim());
    return { start: open, end: close + 1, text: replaced.slice(open, replaced.length - (source.length - close - 1)) };
  }
  const starts = sequence.items.map((item) => {
    const start = lineStart(source, item.range[0]);
    if (!/^[ \t]*-[ \t]+$/.test(source.slice(start, item.range[0]))) refuse();
    return start;
  });
  const end = sequence.items.at(-1).range[2], { at, remove, insert } = splice;
  if (remove && remove === starts.length) {
    const colon = source.lastIndexOf(":", starts[0]);
    if (colon < 0 || source.slice(colon + 1, starts[0]).trim()) refuse();
    return { start: colon + 1, end, text: " []" + eol };
  }
  if (remove) return { start: starts[at], end: at + remove < starts.length ? starts[at + remove] : end, text: "" };
  const indent = source.slice(starts[0], sequence.items[0].range[0]).replace(/-[ \t]*$/, "");
  const text = yamlText(insert, { collectionStyle: "block" }).split(/\r?\n/).map((line) => indent + line + eol).join("");
  const position = at < starts.length ? starts[at] : end;
  return { start: position, end: position, text: (source.slice(0, position).endsWith("\n") ? "" : eol) + text };
}

const valueAt = (value, path) => path.reduce((current, key) => current?.[key], value);

function patchYaml(source, fields, unset, schema) {
  const document = parseDocument(source, { keepSourceTokens: true, uniqueKeys: true });
  if (document.errors.length) throw new PatchError(document.errors[0].message);
  if (!isMap(document.contents)) throw new PatchError("Expected frontmatter mapping");
  const previous = document.toJS({ maxAliasCount: 100 });
  const next = candidate(previous, fields, unset);
  validateCandidate(schema, next);
  const eol = source.includes("\r\n") ? "\r\n" : "\n";
  const edits = [], changed = changes(previous, next);
  for (const { path, value, splice } of changed) {
    const node = document.getIn(path, true);
    if (splice) edits.push(spliceYaml(source, document, path, splice, eol));
    else if (node && node.range && value !== undefined) {
      let text;
      if (typeof value === "string" && /[\r\n]/.test(value)) text = JSON.stringify(value);
      else if (isScalar(node) && typeof value === "string" && node.type === "QUOTE_SINGLE") text = `'${value.replaceAll("'", "''")}'`;
      else if (isScalar(node) && typeof value === "string" && node.type === "QUOTE_DOUBLE") text = JSON.stringify(value);
      else if (isScalar(node) && typeof value === "string" && !plainIsString(value)) text = JSON.stringify(value);
      else text = yamlFlow(value);
      const end = node.range[1];
      if (/\r?\n$/.test(source.slice(node.range[0], end))) text += eol;
      edits.push({ start: node.range[0], end, text });
    } else {
      const parentPath = path.slice(0, -1), key = path.at(-1), parent = document.getIn(parentPath, true);
      if (!isMap(parent) || parent.flow) throw new PatchError("Cannot safely add or remove a field in this YAML shape");
      const pair = parent.items.find((pair) => pair.key?.value === key);
      if (value === undefined && pair) {
        const start = source.lastIndexOf("\n", pair.key.range[0] - 1) + 1, end = pair.value?.range?.[2] ?? pair.key.range[2];
        edits.push({ start, end, text: "" });
      } else if (!pair && value !== undefined) {
        const indent = parent.items.length ? parent.items[0].key.range[0] - (source.lastIndexOf("\n", parent.items[0].key.range[0] - 1) + 1) : 0, end = parent.range[2];
        edits.push({ start: end, end, text: `${source.slice(0, end).endsWith("\n") ? "" : eol}${" ".repeat(indent)}${JSON.stringify(key)}: ${yamlFlow(value)}${eol}` });
      } else throw new PatchError("Cannot safely patch this YAML node");
    }
  }
  const comments = yamlComments(document.contents.srcToken);
  if (edits.some((edit) => comments.some((comment) => edit.start < comment.end && edit.end > comment.start))) {
    throw new PatchError("Cannot safely patch YAML without removing a comment; edit this field in Code");
  }
  const result = edits.sort((a, b) => b.start - a.start).reduce((text, edit) => text.slice(0, edit.start) + edit.text + text.slice(edit.end), source);
  const checked = parseDocument(result);
  if (checked.errors.length || !isDeepStrictEqual(checked.toJS(), next)) throw new PatchError("Patch could not preserve YAML structure");
  // Astro parses frontmatter with js-yaml, which reads some plain scalars (dates) differently.
  let astro;
  try {
    astro = jsyaml.load(result);
  } catch (error) {
    throw new PatchError(`Patched frontmatter is not readable by Astro: ${error.message}`);
  }
  if (changed.some(({ path }) => !isDeepStrictEqual(valueAt(astro, path), valueAt(next, path)))) throw new PatchError("Patch would change how Astro reads these values; edit this field in Code");
  return result;
}

/**
 * Patch a content source while preserving its formatting.
 *
 * `fields` is a partial top-level update and `unset` names top-level keys to
 * remove; an omitted `body` keeps the existing Markdown body, while `body: ""`
 * clears it. The caller resolves `kind` and `schema`; this function never reads
 * files or knows collection names. The complete candidate is validated before
 * any source edit, and unsafe shapes are refused rather than reformatted.
 */
export function patchSource({ source, fields, unset, body }, { kind, schema }) {
  if (kind !== "json" && kind !== "markdown" && kind !== "mdx") throw new PatchError(`Unsupported content kind: ${kind}`);
  if (!fields || typeof fields !== "object" || Array.isArray(fields)) throw new PatchError("fields must be an object");
  const removed = unset ?? [];
  if (!Array.isArray(removed) || removed.some((key) => typeof key !== "string" || Object.hasOwn(fields, key))) throw new PatchError("unset must be top-level field names absent from fields");
  if (kind === "json") return patchJson(source, fields, removed, schema);

  const match = /^(---\r?\n)([\s\S]*?)(^---(?:\r?\n|$))/m.exec(source);
  if (!match || match.index !== 0) throw new PatchError("Missing YAML frontmatter");
  const nextBody = body === undefined ? source.slice(match[0].length) : body;
  if (typeof nextBody !== "string") throw new PatchError("body must be a string");
  return match[1] + patchYaml(match[2], fields, removed, schema) + match[3] + nextBody;
}
