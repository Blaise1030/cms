import type { ConflictChoice } from "@/core/types";

/** "launch-week" → "Launch week". */
function humanize(slug: string): string {
  const words = slug.replace(/[-_]+/g, " ").trim();
  return words ? words[0]!.toUpperCase() + words.slice(1) : slug;
}

/**
 * A writer-facing name for a file in a combine: the page or section it stands for, never its path.
 * Falls back to the bare file name for files that are not pages, sections or content entries.
 */
export function conflictLabel(path: string): string {
  const section = /^src\/content\/sections\/([^/]+)\.mdx?$/.exec(path);
  if (section) return `${humanize(section[1]!)} section`;
  const entry = /^src\/content\/([^/]+)\/(?:[a-z]{2}(?:-[A-Za-z]{2})?\/)?(.+?)\.(?:md|mdx|json|ya?ml)$/.exec(path);
  if (entry) return `${humanize(entry[1]!)} · ${humanize(entry[2]!.split("/").at(-1)!)}`;
  const page = /^(?:src\/pages\/)?(.*?)(?:\/?index)?\.(?:astro|html|md|mdx)$/.exec(path);
  if (page && (path.startsWith("src/pages/") || !path.includes("/"))) {
    return page[1] ? `${humanize(page[1].split("/").at(-1)!)} page` : "Home page";
  }
  return path.split("/").at(-1) ?? path;
}

/** "the Hero section" for one file, "3 parts of your site" for several. */
export function conflictSubject(paths: string[]): string {
  if (paths.length === 1) return `the ${conflictLabel(paths[0]!)}`;
  return `${paths.length} parts of your site`;
}

export const CHOICE_LABEL: Record<ConflictChoice, string> = {
  yours: "Your version",
  latest: "Live site version",
  both: "Both versions",
};
