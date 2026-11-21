/**
 * The site's languages. English is the source and has no URL prefix (`/blog/`); every other
 * locale is served under its key (`/zh/blog/`, `/ms/blog/`).
 *
 * The key doubles as the Crowdin folder and file name (`%two_letters_code%` in crowdin.yml):
 * `src/content/blog/zh/*.md` and `src/i18n/messages/zh.json`.
 */
/**
 * `flag` is the language picker's icon, from the circle-flags set. Write the full
 * `i-circle-flags-<country>` name so the icon bundler finds it.
 */
export const locales = {
  en: { label: "English", lang: "en", intl: "en-US", flag: "i-circle-flags-us" },
  zh: { label: "简体中文", lang: "zh-Hans", intl: "zh-CN", flag: "i-circle-flags-cn" },
  ms: { label: "Bahasa Melayu", lang: "ms", intl: "ms-MY", flag: "i-circle-flags-my" },
} as const;

export type Locale = keyof typeof locales;

export const defaultLocale: Locale = "en";

/** Every locale key, default first. */
export const localeKeys = Object.keys(locales) as Locale[];

/** The locales that are translated from English (every one but the default). */
export const translatedLocales = localeKeys.filter((locale) => locale !== defaultLocale);

export const isLocale = (value: unknown): value is Locale =>
  typeof value === "string" && Object.hasOwn(locales, value);
