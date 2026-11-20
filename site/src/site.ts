/** Site-wide name, description, navigation and footer, rendered by `layouts/Base.astro`. */
export const site = {
  name: "Project Orso",
  /** Short promise, used after the name in the home page title. */
  tagline: "Developer tools for content teams",
  /** Default meta description, social card text and llms.txt summary (keep it under 160 characters). */
  description:
    "Let your content team edit the site with an agent. Drafts, previews and Publish — content stays in your repo.",
  /** Placeholder live address. The build uses PUBLIC_SITE_URL when it is set; replace this with your domain. */
  url: "https://example.com",
  /** Default social card, served from `public/`. */
  ogImage: "/og-default.png",
  nav: [
    { label: "Product", href: "#product" },
    { label: "How it works", href: "#how-it-works" },
    { label: "FAQ", href: "#faq" },
    { label: "Blog", href: "/blog/" },
    { label: "Help", href: "/help/" },
  ],
  /** Header secondary action; set to `null` to hide it. */
  secondaryCta: { label: "GitHub", href: "https://github.com/Blaise1030/cms" },
  /** Header call to action; set to `null` to hide it. */
  cta: { label: "Get Orso", href: "#get-orso" },
  /** Download button: bump `version` when a new GitHub release ships; asset names must match the release. */
  release: {
    version: "v0.1.9",
    repo: "https://github.com/Blaise1030/cms",
    /** Shown in the thank-you popup after a download. Placeholder: replace with your real page. */
    coffeeUrl: "https://buymeacoffee.com/blaise1030",
    downloads: [
      { os: "mac", label: "macOS", detail: "Apple Silicon", target: "aarch64-apple-darwin" },
      { os: "mac", label: "macOS", detail: "Intel", target: "x86_64-apple-darwin" },
      { os: "linux", label: "Linux", detail: "x64", target: "x86_64-unknown-linux-gnu" },
      { os: "linux", label: "Linux", detail: "ARM64", target: "aarch64-unknown-linux-gnu" },
    ],
  },
  footer: {
    text: "© 2026 Project Orso",
    links: [
      { label: "Privacy", href: "/privacy/" },
      { label: "Terms", href: "/terms/" },
    ],
    source: { label: "Open source", href: "https://github.com/Blaise1030/cms" },
    license: { label: "MIT licence", href: "https://github.com/Blaise1030/cms/blob/main/LICENSE" },
    /** Footer-only links, appended to the column of the same name after the header nav links. */
    extra: {
      Product: [
        { label: "Download", href: "#cta" },
        { label: "Releases", href: "https://github.com/Blaise1030/cms/releases" },
      ],
      Resources: [
        { label: "Blog topics", href: "/blog/categories/" },
        { label: "Help topics", href: "/help/categories/" },
        { label: "llms.txt", href: "/llms.txt" },
      ],
      More: [
        { label: "Report an issue", href: "https://github.com/Blaise1030/cms/issues" },
        { label: "Buy me a coffee", href: "https://buymeacoffee.com/blaise1030" },
      ],
    } as Record<string, { label: string; href: string }[]>,
  },
  socials: [
    { label: "GitHub", href: "https://github.com/Blaise1030/cms", icon: "github" as const },
    { label: "Portfolio", href: "https://blaisetiong.dev", icon: "globe" as const },
  ],
};
