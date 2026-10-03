import { describe, expect, it } from "vitest";
import { createMemoryHistory, createRouter } from "vue-router";
import { projectPath, routes } from "./router";

const router = () => createRouter({ history: createMemoryHistory(), routes });

describe("router", () => {
  it("builds project paths with the id encoded", () => {
    expect(projectPath("my-site")).toBe("/p/my-site");
    expect(projectPath("a b/c")).toBe("/p/a%20b%2Fc");
  });

  it("resolves a project path back to its id", () => {
    const r = router().resolve(projectPath("a b/c"));
    expect(r.name).toBe("project");
    expect(r.params.id).toBe("a b/c");
  });

  it("redirects unknown paths home", async () => {
    const r = router();
    await r.push("/nope/deeper");
    expect(r.currentRoute.value.path).toBe("/");
  });
});
