/**
 * Section files have stable IDs in the Astro template; shared files compare as whole pages. The
 * daemon draws the highlight into the built page itself (`?section=`): the frame is
 * cross-origin, so the app cannot reach into it.
 */
export function sectionIdForFile(filePath: string): string | null {
  return /^src\/content\/sections\/([a-z0-9-]+)\.mdx$/.exec(filePath)?.[1] ?? null;
}
