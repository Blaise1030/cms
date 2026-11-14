/** Site-relative preview URLs for the marketing template collections. */
export const marketingPreviewPaths = {
  blog: ({ slug }) => `/blog/${slug}/`,
  helpDocs: ({ slug }) => `/help/${slug}/`,
};

/**
 * Wrap Astro collection definitions with marketing `previewPath` handlers.
 * @param {{ blog: unknown, helpDocs: unknown }} collections
 */
export function attachMarketingPreviewPaths(collections) {
  return {
    blog: { collection: collections.blog, previewPath: marketingPreviewPaths.blog },
    helpDocs: { collection: collections.helpDocs, previewPath: marketingPreviewPaths.helpDocs },
  };
}