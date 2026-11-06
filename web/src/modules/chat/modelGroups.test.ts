import { describe, expect, it } from "vitest";
import { groupModels } from "./modelGroups";

describe("groupModels", () => {
  it("puts Default alone, then slash models under their provider prefix", () => {
    expect(
      groupModels([
        { id: "default", name: "Default" },
        { id: "deepseek/deepseek-flash", name: "deepseek/deepseek-flash" },
        { id: "deepseek/deepseek-v4-pro", name: "deepseek/deepseek-v4-pro" },
        { id: "opencode-go/beam-free", name: "opencode-go/beam-free" },
      ]),
    ).toEqual([
      {
        heading: null,
        items: [{ id: "default", name: "Default", label: "Default" }],
      },
      {
        heading: "deepseek",
        items: [
          { id: "deepseek/deepseek-flash", name: "deepseek/deepseek-flash", label: "deepseek-flash" },
          { id: "deepseek/deepseek-v4-pro", name: "deepseek/deepseek-v4-pro", label: "deepseek-v4-pro" },
        ],
      },
      {
        heading: "opencode-go",
        items: [{ id: "opencode-go/beam-free", name: "opencode-go/beam-free", label: "beam-free" }],
      },
    ]);
  });

  it("keeps models without a slash under the ungrouped section", () => {
    expect(
      groupModels([
        { id: "default", name: "Default" },
        { id: "gpt-5", name: "GPT-5" },
        { id: "acme/fast", name: "acme/fast" },
      ]),
    ).toEqual([
      {
        heading: null,
        items: [
          { id: "default", name: "Default", label: "Default" },
          { id: "gpt-5", name: "GPT-5", label: "GPT-5" },
        ],
      },
      {
        heading: "acme",
        items: [{ id: "acme/fast", name: "acme/fast", label: "fast" }],
      },
    ]);
  });

  it("preserves first-seen provider order", () => {
    const headings = groupModels([
      { id: "z/one", name: "z/one" },
      { id: "a/two", name: "a/two" },
      { id: "z/three", name: "z/three" },
    ]).map((g) => g.heading);
    expect(headings).toEqual(["z", "a"]);
  });
});
