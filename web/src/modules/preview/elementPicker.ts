/**
 * An element picked in the preview, described for the agent. The preview is cross-origin, so
 * the page's bridge (`packages/astro/src/bridge.js`) runs the picker and describes the element;
 * the app only turns the description into a mention.
 */
export interface PickedElement {
  /** Short display label: `h1.title`, `section#pricing`. */
  label: string;
  /** The element's opening tag, trimmed: `<h1 class="title">`. */
  tag: string;
  /** A CSS path from the nearest id (or `body`) to the element. */
  selector: string;
  /** Visible text, collapsed and truncated; empty for elements without text. */
  text: string;
  /** Source file of the nearest Astro-annotated ancestor (dev mode only), relative when possible. */
  file?: string;
  /** `line:col` in `file`. */
  loc?: string;
  /**
   * Absolute path of a screenshot of the element, saved by the daemon for the agent. (From the
   * bridge it arrives as a PNG data URL, which the panel uploads and replaces.)
   */
  screenshot?: string;
}

/** `/Users/me/orso-projects/site-1/src/pages/index.astro` → `src/pages/index.astro`. */
export function relativeFile(file: string, projectId: string): string {
  const marker = `/${projectId}/`;
  const at = file.lastIndexOf(marker);
  return at >= 0 ? file.slice(at + marker.length) : file;
}

/** The one-line reference the agent receives in place of the composer chip. */
export function elementPrompt(pick: PickedElement): string {
  const where = pick.file
    ? `in ${pick.file}${pick.loc ? `:${pick.loc}` : ""}`
    : `at \`${pick.selector}\``;
  const text = pick.text ? ` "${pick.text}"` : "";
  // Outside the brackets and escaped like a dropped file, so agent CLIs attach the image.
  const shot = pick.screenshot ? ` ${pick.screenshot.replace(/ /g, "\\ ")} ` : "";
  return `[element ${pick.tag}${text} ${where}]${shot}`;
}
