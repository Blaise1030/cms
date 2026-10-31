import { expect, it } from "vitest";
import { humanize } from "./labels";

it("turns camelCase collection names into sentence case", () => {
  expect(humanize("blogCategories")).toBe("Blog categories");
  expect(humanize("helpDocsCategories")).toBe("Help docs categories");
  expect(humanize("helpCategories")).toBe("Help categories");
  expect(humanize("helpSubcategories")).toBe("Help subcategories");
  expect(humanize("helpDocs")).toBe("Help docs");
});

it("turns kebab-case entry slugs into sentence case", () => {
  expect(humanize("getting-started")).toBe("Getting started");
  expect(humanize("content")).toBe("Content");
  expect(humanize("publishing")).toBe("Publishing");
});

it("keeps acronyms and leaves an already-plain name alone", () => {
  expect(humanize("SEO")).toBe("SEO");
  expect(humanize("apiKeys")).toBe("Api keys");
  expect(humanize("APIKeys")).toBe("API keys");
  expect(humanize("blog")).toBe("Blog");
});

it("keeps snake_case words apart and never returns an empty string", () => {
  expect(humanize("help_docs")).toBe("Help docs");
  expect(humanize("")).toBe("");
});
