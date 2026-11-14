export type ResourceSummary = {
  scripts: number;
  styles: number;
  images: number;
  htmlBytes: number;
};

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function resourceSummary(doc: Document): ResourceSummary {
  return {
    scripts: doc.querySelectorAll("script").length,
    styles: doc.querySelectorAll('link[rel="stylesheet"], style').length,
    images: doc.querySelectorAll("img").length,
    htmlBytes: new TextEncoder().encode(doc.documentElement.outerHTML).length,
  };
}

export function formatResourceSummary(summary: ResourceSummary): string {
  const { scripts, styles, images, htmlBytes } = summary;
  return `${scripts} scripts · ${styles} styles · ${images} images · HTML ${formatBytes(htmlBytes)}`;
}
