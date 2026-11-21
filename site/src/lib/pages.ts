import { loadBlog } from "./blog";
import { loadHelp } from "./help";
import { localeKeys, localePath } from "../i18n";

export interface SitePage {
  /** Site path, e.g. `/blog/welcome/`. */
  path: string;
  /** Last change (YYYY-MM-DD), when known. */
  lastmod?: string;
}

/**
 * Paths of the static `.astro` pages in src/pages (no `[param]` routes, no `_` files, no 404),
 * so a page you add shows up in the sitemap without editing this file. Pages under
 * `[...locale]/` count once here and are listed per language below.
 */
const staticPaths = () =>
  Object.keys(import.meta.glob("../pages/**/*.astro"))
    .map((file) => file.replace(/^\.\.\/pages/, "").replace(/^\/\[\.\.\.locale\]/, "").replace(/\.astro$/, ""))
    .filter((route) => !/\[|\/_|^\/404$/.test(route))
    .map((route) => route.replace(/\/index$/, "/").replace(/^$/, "/").replace(/([^/])$/, "$1/"));

/** Every page of the built site, in every language: static pages, blog posts and categories, help docs, sections and topics. */
export async function sitePages(): Promise<SitePage[]> {
  const perLocale = await Promise.all(
    localeKeys.map(async (locale) => {
      const [{ posts, categories }, { docs, sections, topics }] = await Promise.all([loadBlog(locale), loadHelp(locale)]);
      const pages: SitePage[] = [
        ...staticPaths().map((path) => ({ path })),
        ...posts.map((post) => ({ path: `/blog/${post.id}/`, lastmod: post.data.pubDate })),
        ...categories.map((category) => ({ path: `/blog/categories/${category.id}/`, lastmod: category.posts[0]?.data.pubDate })),
        ...docs.map((doc) => ({ path: `/help/${doc.id}/` })),
        ...sections.map((section) => ({ path: `/help/section/${section.id}/` })),
        ...topics.map((topic) => ({ path: `/help/categories/${topic.id}/` })),
      ];
      return pages.map((page) => ({ ...page, path: localePath(locale, page.path) }));
    }),
  );
  const seen = new Set<string>();
  return perLocale
    .flat()
    .filter((page) => !seen.has(page.path) && seen.add(page.path))
    .sort((a, b) => (a.path === "/" ? -1 : b.path === "/" ? 1 : a.path.localeCompare(b.path)));
}
