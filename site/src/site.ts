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
  secondaryCta: { label: "See what it does", href: "#scale" },
  /** Header call to action; set to `null` to hide it. */
  cta: { label: "Get Project Orso", href: "https://github.com/Blaise1030/cms" },
  footer: {
    text: "© 2026 Project Orso. Free and open source under the MIT licence.",
    links: [
      { label: "Privacy", href: "/privacy/" },
      { label: "Terms", href: "/terms/" },
    ],
  },
  socials: [
    { label: "GitHub", href: "https://github.com/Blaise1030/cms" },
    { label: "Blaise", href: "https://blaisetiong.dev" },
  ],
};
