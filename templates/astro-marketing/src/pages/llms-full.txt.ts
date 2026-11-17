import type { APIRoute } from "astro";
import { loadBlog } from "../lib/blog";
import { loadHelp } from "../lib/help";
import { absoluteUrl } from "../lib/seo";
import { site } from "../site";

export const prerender = true;

/** The Markdown source of every help article and blog post, for language models (see llms.txt). */
export const GET: APIRoute = async () => {
  const [{ posts }, { docs }] = await Promise.all([loadBlog(), loadHelp()]);
  const article = (title: string, path: string, description: string, body = "") =>
    `## ${title}\n\nSource: ${absoluteUrl(path)}\n\n${description}\n\n${body.trim()}`;
  const blocks = [
    `# ${site.name}`,
    `> ${site.description}`,
    ...docs.map((doc) => article(doc.data.title, `/help/${doc.id}/`, doc.data.description, doc.body)),
    ...posts.map((post) => article(post.data.title, `/blog/${post.id}/`, post.data.description, post.body)),
  ];
  return new Response(`${blocks.join("\n\n")}\n`, { headers: { "Content-Type": "text/plain; charset=utf-8" } });
};
