/**
 * Site translations. Copy lives in `messages/<locale>.json`: `en.json` is the source Crowdin
 * translates, and Crowdin writes `zh.json`, `ms.json` and so on next to it. Any key a locale
 * hasn't translated yet falls back to English, so a page never shows a blank or a raw key.
 *
 * In an Astro component:
 *   const { t, locale, path } = getI18n(Astro.url);
 *   <a href={path("/blog/")}>{t("nav.blog")}</a>
 */
import en from "./messages/en.json";
import { defaultLocale, isLocale, localeKeys, locales, type Locale } from "./locales";

export * from "./locales";

type Messages = typeof en;
type Plural = { one: string; other: string };
type Tree = { [key: string]: string | Tree };

/** Every dotted key that resolves to a string or a plural pair, e.g. "nav.blog" or "blog.postCount". */
type Paths<T> = {
  [K in keyof T & string]: T[K] extends string | Plural ? K : `${K}.${Paths<T[K]>}`;
}[keyof T & string];
export type MessageKey = Paths<Messages>;

const catalogs = import.meta.glob<Tree>("./messages/*.json", { eager: true, import: "default" });
const catalog = (locale: Locale): Tree | undefined => catalogs[`./messages/${locale}.json`];

const lookup = (tree: Tree | undefined, key: string): string | Tree | undefined =>
  key.split(".").reduce<string | Tree | undefined>((node, part) => (node && typeof node === "object" ? node[part] : undefined), tree);

/** Crowdin can export untranslated strings as empty; treat those as missing. */
const present = <T>(value: T) => (value === "" ? undefined : value);

/** English with the locale's translations laid over it, key by key. */
function overlay(source: string | Tree | undefined, translated: string | Tree | undefined): string | Tree | undefined {
  if (typeof source === "string" || source === undefined) return typeof translated === "string" ? present(translated) ?? source : source;
  const own = translated && typeof translated === "object" ? translated : {};
  return Object.fromEntries(Object.entries(source).map(([key, value]) => [key, overlay(value, own[key])]));
}

const fill = (text: string, vars: Record<string, string | number>) =>
  text.replace(/\{(\w+)\}/g, (match, name: string) => (name in vars ? String(vars[name]) : match));

export function useTranslations(locale: Locale) {
  const own = catalog(locale);
  const plurals = new Intl.PluralRules(locales[locale].intl);
  const resolve = (key: string) => overlay(lookup(en as Tree, key), lookup(own, key));

  /** The message for `key`, with `{name}` placeholders filled. Plural keys need a `count`. */
  const t = (key: MessageKey, vars: Record<string, string | number> = {}): string => {
    let value = resolve(key);
    if (value && typeof value === "object" && "count" in vars) {
      value = value[plurals.select(Number(vars.count))] ?? value.other;
    }
    if (typeof value !== "string") throw new Error(`i18n: "${key}" is not a message in messages/en.json`);
    return fill(value, vars);
  };

  /** A whole group of messages (English where untranslated), for handing to a Vue island as props. */
  const group = <T = Record<string, unknown>>(key: string): T => {
    const value = resolve(key);
    if (!value || typeof value !== "object") throw new Error(`i18n: "${key}" is not a group in messages/en.json`);
    return value as T;
  };

  const date = (value: string, month: "short" | "long" = "short") =>
    new Date(`${value}T00:00:00Z`).toLocaleDateString(locales[locale].intl, { month, day: "numeric", year: "numeric", timeZone: "UTC" });

  return {
    locale,
    lang: locales[locale].lang,
    t,
    group,
    date,
    /** A site path in this locale, ready for an `href`. */
    path: (target: string) => localeHref(locale, target),
    /** A site path in this locale, as `localePath` gives it (for `absoluteUrl` and breadcrumbs). */
    sitePath: (target: string) => localePath(locale, target),
  };
}

/** Site paths that exist in every language. Anything else (llms.txt, feeds) is English-only. */
export const isLocalized = (path: string) => path === "/" || path.startsWith("#") || /^\/(blog|help|privacy|terms)(\/|$)/.test(path);

/** A site path in `locale`: `/blog/` → `/zh/blog/`, `#faq` → `/zh/#faq`. External links pass through. */
export function localePath(locale: Locale, path: string) {
  if (locale === defaultLocale || !isLocalized(path)) return path;
  return path.startsWith("#") ? `/${locale}/${path}` : `/${locale}${path}`;
}

/** `localePath` with home-page anchors pointed at the home page (`#faq` → `/#faq`), for an `href`. */
export const localeHref = (locale: Locale, path: string) => {
  const target = localePath(locale, path);
  return target.startsWith("#") ? `/${target}` : target;
};

/** The page's path without a locale prefix: `/zh/blog/` → `/blog/`. */
export function unlocalizedPath(url: URL) {
  const rest = url.pathname.replace(/^\//, "");
  const [first, ...tail] = rest.split("/");
  return `/${isLocale(first) && first !== defaultLocale ? tail.join("/") : rest}`;
}

/** The locale a URL is in, from its first path segment. */
export function localeFromUrl(url: URL): Locale {
  const first = url.pathname.replace(/^\//, "").split("/")[0];
  return isLocale(first) ? first : defaultLocale;
}

/** Translations for the page being rendered: `getI18n(Astro.url)`. */
export const getI18n = (url: URL) => useTranslations(localeFromUrl(url));

/** The `[...locale]` param for a locale: absent for English, so English URLs keep no prefix. */
export const localeParam = (locale: Locale) => (locale === defaultLocale ? undefined : locale);

/** `getStaticPaths` for a page with no other params: one copy per locale. */
export const localeStaticPaths = () => localeKeys.map((locale) => ({ params: { locale: localeParam(locale) } }));

/**
 * `getStaticPaths` for a page with its own params, once per locale. `build` returns the paths
 * for one locale; the `locale` param is added for you.
 *   export const getStaticPaths = () => localizedStaticPaths(async (locale) => …);
 */
export async function localizedStaticPaths<P extends { params: Record<string, string | undefined> }>(build: (locale: Locale) => Promise<P[]>) {
  const perLocale = await Promise.all(
    localeKeys.map(async (locale) => (await build(locale)).map((path) => ({ ...path, params: { ...path.params, locale: localeParam(locale) } }))),
  );
  return perLocale.flat();
}
