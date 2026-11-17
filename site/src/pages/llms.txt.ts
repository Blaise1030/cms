import type { APIRoute } from "astro";
import { loadBlog } from "../lib/blog";
import { loadHelp } from "../lib/help";
import { absoluteUrl } from "../lib/seo";
import { site } from "../site";

export const prerender = true;

const link = (title: string, path: string, description?: string) =>
  `- [${title}](${absoluteUrl(path)})${description ? `: ${description}` : ""}`;

/** A Markdown map of the site for language models, in the llmstxt.org format. */
export const GET: APIRoute = async () => {
  const [{ posts }, { sections }] = await Promise.all([loadBlog(), loadHelp()]);
  const blocks = [
    `# ${site.name}`,
    `> ${site.description}`,
    `Help articles explain how to use ${site.name}; blog posts cover product news and guides. The full text of every article is in [llms-full.txt](${absoluteUrl("/llms-full.txt")}).`,
    ["## Pages", link("Home", "/", site.description), link("Help", "/help/", "Guides and answers, from your first sign-in to your next invoice."), link("Blog", "/blog/", "Product news, practical guides and stories from the team.")].join("\n"),
    ...sections
      .filter((section) => section.docs.length > 0)
      .map((section) => [`## Help: ${section.title}`, ...section.docs.map((doc) => link(doc.data.title, `/help/${doc.id}/`, doc.data.description))].join("\n")),
    posts.length > 0 ? ["## Blog", ...posts.map((post) => link(post.data.title, `/blog/${post.id}/`, post.data.description))].join("\n") : "",
    ["## Optional", link("Privacy policy", "/privacy/"), link("Terms of service", "/terms/")].join("\n"),
  ];
  return new Response(`${blocks.filter(Boolean).join("\n\n")}\n`, { headers: { "Content-Type": "text/plain; charset=utf-8" } });
};
