import type { FieldDescriptor } from "@/core/types";
import { cloneValue } from "./draft";

/** A URL-safe slug from a title: lowercase letters and digits joined by single hyphens. */
export function slugify(title: string): string {
  return title
    .normalize("NFKD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 100)
    .replace(/-+$/, "");
}

/** Today as `YYYY-MM-DD` in local time. */
export function today(now = new Date()): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

type Start = { value: unknown } | { skip: true } | { reason: string };

function start(d: FieldDescriptor, key: string, title: string, date: string): Start {
  if (d.default !== undefined) return { value: cloneValue(d.default) };
  if (!d.required) return { skip: true };
  switch (d.type) {
    case "string": return { value: key === "title" ? title : "" };
    case "date": return { value: date };
    case "number": return { value: 0 };
    case "boolean": return { value: false };
    case "array": return { value: [] };
    case "enum": case "literal":
      return d.values?.length ? { value: d.values[0] } : { reason: `${key} has no value to start from` };
    case "object": return object(d, key, title, date);
    case "union": {
      const first = d.variants?.[0];
      return first ? start({ ...first, required: true }, key, title, date) : { reason: `${key} has no variant to start from` };
    }
    default:
      return { reason: `${key || "This entry"} is required and has no default, so it cannot be created here` };
  }
}

function object(d: FieldDescriptor, path: string, title: string, date: string): Start {
  const value: Record<string, unknown> = {};
  for (const [name, child] of Object.entries(d.properties ?? {})) {
    const next = start(child, path ? `${path}.${name}` : name, title, date);
    if ("reason" in next) return next;
    if ("value" in next) value[name] = next.value;
  }
  return { value };
}

/**
 * The starting fields of a new entry: schema defaults, the title, today for dates, and empty
 * values for other required fields. A required field that cannot be filled (an image, a
 * reference, an unsupported node) without a default makes the collection non-creatable.
 */
export function startingFields(d: FieldDescriptor, title: string, date = today()): { fields: Record<string, unknown> } | { reason: string } {
  const result = start({ ...d, required: true }, "", title, date);
  if ("reason" in result) return result;
  const value = "value" in result ? result.value : {};
  if (!value || typeof value !== "object" || Array.isArray(value)) return { reason: "This collection's entries are not objects" };
  return { fields: value as Record<string, unknown> };
}
