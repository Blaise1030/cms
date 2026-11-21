/**
 * Sätteri hast plugin: makes root-relative links and images in Markdown base-aware.
 *
 * Content writers link to other pages the plain way, `[Install orso](/help/install-orso/)`.
 * In orso's live preview the site is served under `/preview/{id}/` (`base` in astro.config.mjs),
 * so a bare `/help/...` would leave the preview. This prefixes every `href`/`src` that starts
 * with a single `/` with the base. Protocol-relative (`//cdn…`), absolute, relative and `#hash`
 * URLs are left alone.
 *
 * @param {{ base: string }} options
 */
export function baseLinks({ base }) {
  const prefix = base.replace(/\/+$/, "");
  const fix = (url) =>
    prefix && typeof url === "string" && url.startsWith("/") && !url.startsWith("//") && !url.startsWith(`${prefix}/`)
      ? `${prefix}${url}`
      : undefined;

  return {
    name: "base-links",
    element: {
      filter: ["a", "img"],
      visit(node, ctx) {
        const attr = node.tagName === "a" ? "href" : "src";
        const url = fix(node.properties?.[attr]);
        if (url) ctx.setProperty(node, attr, url);
      },
    },
  };
}
