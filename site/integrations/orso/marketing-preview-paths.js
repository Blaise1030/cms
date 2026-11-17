/**
 * Site-relative preview URLs for the marketing template collections.
 * `slug` is always the entry id (the Markdown filename without extension).
 */
export const marketingPreviewPaths = {
  blog: ({ slug }) => `/blog/${slug}/`,
  blogCategories: ({ slug }) => `/blog/categories/${slug}/`,
  helpDocs: ({ slug }) => `/help/${slug}/`,
  helpCategories: ({ slug }) => `/help/section/${slug}/`,
  helpDocsCategories: ({ slug }) => `/help/categories/${slug}/`,
};

/**
 * Wrap Astro collection definitions as `{ collection, previewPath? }` entries for
 * `orso({ collections })`. Collections with a known marketing route get its `previewPath`;
 * any other collection (for example `helpSubcategories`, which has no page of its own) is
 * passed through without one.
 * @param {Record<string, unknown>} collections
 */
export function attachMarketingPreviewPaths(collections) {
  return Object.fromEntries(
    Object.entries(collections).map(([name, collection]) => {
      const previewPath = Object.hasOwn(marketingPreviewPaths, name) ? marketingPreviewPaths[name] : undefined;
      return [name, previewPath ? { collection, previewPath } : { collection }];
    }),
  );
}
