/** Site-wide name, description, navigation and footer, rendered by `layouts/Base.astro`. */
export const site = {
  name: "Acme",
  /** Short promise, used after the name in the home page title. */
  tagline: "Plan your team's work on one page",
  /** Default meta description, social card text and llms.txt summary (keep it under 160 characters). */
  description:
    "Acme keeps projects, milestones and the reasons behind each decision in one shared plan, so a small team always knows what ships next.",
  /** Placeholder live address. The build uses PUBLIC_SITE_URL when it is set; replace this with your domain. */
  url: "https://example.com",
  /** Default social card, served from `public/`. */
  ogImage: "/og-default.png",
  nav: [
    { label: "Features", href: "#features" },
    { label: "Pricing", href: "#pricing" },
    { label: "FAQ", href: "#faq" },
    { label: "Blog", href: "/blog/" },
    { label: "Help", href: "/help/" },
  ],
  /** Header call to action; set to `null` to hide it. */
  cta: { label: "Start free", href: "#pricing" },
  footer: {
    text: "© 2026 Acme, Inc. All rights reserved.",
    links: [
      { label: "Privacy", href: "/privacy/" },
      { label: "Terms", href: "/terms/" },
    ],
  },
  socials: [
    { label: "X", href: "https://x.com" },
    { label: "GitHub", href: "https://github.com" },
  ],
};
