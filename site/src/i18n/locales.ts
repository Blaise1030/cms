/**
 * The site's languages. English is the source and has no URL prefix (`/blog/`); every other
 * locale is served under its key (`/zh/blog/`, `/ms/blog/`).
 *
 * The key doubles as the Crowdin folder and file name (`%two_letters_code%` in crowdin.yml):
 * `src/content/blog/zh/*.md` and `src/i18n/messages/zh.json`.
 */
export const locales = {
  en: { label: "English", lang: "en", intl: "en-US" },
  zh: { label: "简体中文", lang: "zh-Hans", intl: "zh-CN" },
  ms: { label: "Bahasa Melayu", lang: "ms", intl: "ms-MY" },
} as const;

export type Locale = keyof typeof locales;

export const defaultLocale: Locale = "en";

/** Every locale key, default first. */
export const localeKeys = Object.keys(locales) as Locale[];

/** The locales that are translated from English (every one but the default). */
export const translatedLocales = localeKeys.filter((locale) => locale !== defaultLocale);

export const isLocale = (value: unknown): value is Locale =>
  typeof value === "string" && Object.hasOwn(locales, value);
