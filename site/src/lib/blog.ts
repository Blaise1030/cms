import { getCollection, type CollectionEntry } from "astro:content";

export type Post = CollectionEntry<"blog">;

/** A blog category with its posts, newest first. `id` is the category file name and URL slug. */
export interface BlogCategory {
  id: string;
  title: string;
  description?: string;
  posts: Post[];
}

/** Newest first; ties break on the file name so the order never depends on the filesystem. */
export const sortPosts = (posts: Post[]) =>
  [...posts].sort((a, b) => b.data.pubDate.localeCompare(a.data.pubDate) || a.id.localeCompare(b.id));

export const formatDate = (value: string) =>
  new Date(`${value}T00:00:00Z`).toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric", timeZone: "UTC" });

/** All posts (newest first) and all categories (A–Z), each with its posts. */
export async function loadBlog() {
  const [posts, categoryEntries] = await Promise.all([getCollection("blog"), getCollection("blogCategories")]);
  const sorted = sortPosts(posts);
  const categories: BlogCategory[] = [...categoryEntries]
    .sort((a, b) => a.data.title.localeCompare(b.data.title) || a.id.localeCompare(b.id))
    .map((category) => ({
      id: category.id,
      title: category.data.title,
      description: category.data.description,
      posts: sorted.filter((post) => post.data.categories.includes(category.id)),
    }));
  return { posts: sorted, categories };
}

/** The categories a post lists, in its own order; ids with no category file are skipped. */
export const categoriesOf = (post: Post, categories: BlogCategory[]) =>
  post.data.categories.flatMap((id) => categories.filter((category) => category.id === id));

/** Lower-case text the blog search matches: title, description, tags and category titles. */
export const searchText = (post: Post, categories: BlogCategory[]) =>
  [post.data.title, post.data.description, ...post.data.tags, ...categoriesOf(post, categories).map((category) => category.title)]
    .join(" ")
    .toLowerCase();

/** Up to `count` posts to read next: the post's `relatedPosts`, then posts sharing a category, then the newest. */
export function morePosts(post: Post, posts: Post[], count = 3) {
  const others = posts.filter((other) => other.id !== post.id);
  const related = post.data.relatedPosts.flatMap((id) => others.filter((other) => other.id === id));
  const sameCategory = others.filter((other) => other.data.categories.some((id) => post.data.categories.includes(id)));
  return [...new Set([...related, ...sameCategory, ...others])].slice(0, count);
}
