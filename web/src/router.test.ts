import { describe, expect, it } from "vitest";
import { createMemoryHistory, createRouter } from "vue-router";
import { codePath, contentPath, historyPath, projectPath, projectTabFromRoute, routes } from "./router";

const router = () => createRouter({ history: createMemoryHistory(), routes });

describe("router", () => {
  it("builds project paths with the id encoded and a tab segment", () => {
    expect(projectPath("my-site")).toBe("/p/my-site/preview");
    expect(projectPath("my-site", "code")).toBe("/p/my-site/code");
    expect(projectPath("a b/c", "history")).toBe("/p/a%20b%2Fc/history");
  });

  it("resolves a project path back to its id and tab", () => {
    const r = router().resolve(projectPath("a b/c", "code"));
    expect(r.name).toBe("project-code");
    expect(r.params.id).toBe("a b/c");
  });

  it("redirects /p/:id to the preview tab", async () => {
    const r = router();
    await r.push("/p/site");
    expect(r.currentRoute.value.name).toBe("project-preview");
    expect(r.currentRoute.value.path).toBe("/p/site/preview");
  });

  it("round-trips a content entry and a nested file path through the URL", () => {
    expect(contentPath("site", "blog", "my post")).toBe("/p/site/content/blog/my%20post");
    const entry = router().resolve(contentPath("site", "blog", "2024/my post"));
    expect(entry.name).toBe("project-content");
    expect(entry.params).toMatchObject({ id: "site", collection: "blog", entry: "2024/my post" });
    expect(router().resolve(contentPath("site", "blog")).params.collection).toBe("blog");

    expect(codePath("site", "src/pages/index.astro")).toBe("/p/site/code/src/pages/index.astro");
    const file = router().resolve(codePath("site", "src/a b.md"));
    expect(file.name).toBe("project-code");
    expect(file.params.path).toBe("src/a b.md");

    expect(historyPath("site", "abc123")).toBe("/p/site/history/abc123");
    const checkpoint = router().resolve(historyPath("site", "abc123"));
    expect(checkpoint.name).toBe("project-history");
    expect(checkpoint.params.sha).toBe("abc123");
    expect(historyPath("site")).toBe("/p/site/history");
  });

  it("maps route names to tabs", () => {
    expect(projectTabFromRoute("project-preview")).toBe("preview");
    expect(projectTabFromRoute("project-code")).toBe("code");
    expect(projectTabFromRoute("project-history")).toBe("history");
    expect(projectTabFromRoute("project-design")).toBe("design");
    expect(projectTabFromRoute("home")).toBe("preview");
  });

  it("resolves the design tab", () => {
    const r = router().resolve("/p/x/design");
    expect(r.name).toBe("project-design");
    expect(r.params.id).toBe("x");
    expect(projectPath("x", "design")).toBe("/p/x/design");
  });

  it("redirects unknown paths home", async () => {
    const r = router();
    await r.push("/nope/deeper");
    expect(r.currentRoute.value.path).toBe("/");
  });
});
