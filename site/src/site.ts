import en from "./i18n/messages/en.json";

/**
 * Site-wide name, links and release details, rendered by `layouts/Base.astro`. Every label the
 * visitor reads lives in `src/i18n/messages/en.json` (and its translations); links here name
 * their label by `key`: `nav.<key>` for header links, `footer.links.<key>` for footer links.
 */
export type FooterLink = { key: keyof typeof en.footer.links; href: string };

export const site = {
  name: "Project Orso",
  /** Short promise, used after the name in the home page title. Edit it in messages/en.json (`site.tagline`). */
  tagline: en.site.tagline,
  /** Default meta description, social card text and llms.txt summary (under 160 characters). Edit it in messages/en.json (`site.description`). */
  description: en.site.description,
  /** Placeholder live address. The build uses PUBLIC_SITE_URL when it is set; replace this with your domain. */
  url: "https://example.com",
  /** Default social card, served from `public/`. */
  ogImage: "/og-default.png",
  nav: [
    { key: "product", href: "#product" },
    { key: "howItWorks", href: "#how-it-works" },
    { key: "faq", href: "#faq" },
    { key: "blog", href: "/blog/" },
    { key: "help", href: "/help/" },
  ],
  /** Header secondary action; set to `null` to hide it. */
  secondaryCta: { key: "github", href: "https://github.com/Blaise1030/cms" },
  /** Header call to action; set to `null` to hide it. */
  cta: { key: "getOrso", href: "#get-orso" },
  /** Download button: bump `version` when a new GitHub release ships; asset names must match the release. */
  release: {
    version: "v0.1.10",
    repo: "https://github.com/Blaise1030/cms",
    /** Shown in the thank-you popup after a download. */
    coffeeUrl: "https://buymeacoffee.com/blaisetionS",
    downloads: [
      { os: "mac", label: "macOS", detail: "Apple Silicon", target: "aarch64-apple-darwin", ext: "dmg" },
      { os: "mac", label: "macOS", detail: "Intel", target: "x86_64-apple-darwin", ext: "dmg" },
      { os: "linux", label: "Linux", detail: "x64", target: "x86_64-unknown-linux-gnu", ext: "tar.gz" },
      { os: "linux", label: "Linux", detail: "ARM64", target: "aarch64-unknown-linux-gnu", ext: "tar.gz" },
    ],
  },
  footer: {
    links: [
      { key: "privacy", href: "/privacy/" },
      { key: "terms", href: "/terms/" },
    ],
    source: { key: "openSource", href: "https://github.com/Blaise1030/cms" },
    license: { key: "licence", href: "https://github.com/Blaise1030/cms/blob/main/LICENSE" },
    /** Footer-only links, appended to the column of the same name after the header nav links. */
    extra: {
      Product: [
        { key: "download", href: "#cta" },
        { key: "releases", href: "https://github.com/Blaise1030/cms/releases" },
      ],
      Resources: [
        { key: "blogTopics", href: "/blog/categories/" },
        { key: "llms", href: "/llms.txt" },
      ],
      More: [
        { key: "reportIssue", href: "https://github.com/Blaise1030/cms/issues" },
        { key: "coffee", href: "https://buymeacoffee.com/blaisetionS" },
      ],
    } as Record<string, FooterLink[]>,
  },
  socials: [
    { key: "github", href: "https://github.com/Blaise1030/cms", icon: "github" as const },
    { key: "portfolio", href: "https://blaisetiong.dev", icon: "globe" as const },
  ],
};
