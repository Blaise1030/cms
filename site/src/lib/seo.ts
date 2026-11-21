import css from "../styles/global.css?raw";
import { site } from "../site";

/** The live site's origin (astro.config.mjs `site`, from PUBLIC_SITE_URL or `site.url`). */
const origin = () => import.meta.env.SITE || site.url;

/** Absolute URL for a site path such as `/blog/welcome/`, on the live site. */
export const absoluteUrl = (path = "/") => new URL(path, origin()).href;

/** The surface colour in each mode, read from the design tokens so `theme-color` never drifts. */
export const themeColors = (() => {
  const light = css.match(/@theme[^{]*\{[\s\S]*?--color-surface:\s*([^;]+);/)?.[1].trim();
  const dark = css.match(/prefers-color-scheme:\s*dark[\s\S]*?--color-surface:\s*([^;]+);/)?.[1].trim();
  return { light, dark: dark ?? light };
})();

type Crumb = { name: string; path: string };

/** schema.org BreadcrumbList for a trail of pages, home first. */
export const breadcrumbs = (trail: Crumb[]) => ({
  "@context": "https://schema.org",
  "@type": "BreadcrumbList",
  itemListElement: trail.map((crumb, index) => ({
    "@type": "ListItem",
    position: index + 1,
    name: crumb.name,
    item: absoluteUrl(crumb.path),
  })),
});

/** schema.org Organization for the company behind the site. */
export const organization = () => ({
  "@context": "https://schema.org",
  "@type": "Organization",
  "@id": absoluteUrl("/#organization"),
  name: site.name,
  url: absoluteUrl("/"),
  logo: absoluteUrl("/apple-touch-icon.png"),
  sameAs: site.socials.map((social) => social.href),
});

/** schema.org WebSite for the home page. */
export const website = () => ({
  "@context": "https://schema.org",
  "@type": "WebSite",
  "@id": absoluteUrl("/#website"),
  name: site.name,
  description: site.description,
  url: absoluteUrl("/"),
  publisher: { "@id": absoluteUrl("/#organization") },
});
